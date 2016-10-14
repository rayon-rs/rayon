//! Internal traits and functions used to implement parallel
//! iteration. These should be considered highly unstable: users of
//! parallel iterators should not need to interact with them directly.
//! See `README.md` for a high-level overview.

use join;
use super::IndexedParallelIterator;
use super::len::*;
use thread_pool::get_registry;

pub trait ProducerCallback<ITEM> {
    type Output;
    fn callback<P>(self, producer: P) -> Self::Output
        where P: Producer<Item=ITEM>;
}

/// A producer which will produce a fixed number of items N. This is
/// not queryable through the API; the consumer is expected to track
/// it.
pub trait Producer: IntoIterator + Send + Sized {
    /// Reports whether the producer has explicit weights.
    fn weighted(&self) -> bool { false }

    /// Cost to produce `len` items, where `len` must be `N`.
    fn cost(&mut self, len: usize) -> f64;

    /// Split into two producers; one produces items `0..index`, the
    /// other `index..N`. Index must be less than `N`.
    fn split_at(self, index: usize) -> (Self, Self);
}

/// A consumer which consumes items that are fed to it.
pub trait Consumer<Item>: Send + Sized {
    type Folder: Folder<Item, Result=Self::Result>;
    type Reducer: Reducer<Self::Result>;
    type Result: Send;

    /// Reports whether the consumer has explicit weights.
    fn weighted(&self) -> bool { false }

    /// If it costs `producer_cost` to produce the items we will
    /// consume, returns cost adjusted to account for consuming them.
    fn cost(&mut self, producer_cost: f64) -> f64;

    /// Divide the consumer into two consumers, one processing items
    /// `0..index` and one processing items from `index..`. Also
    /// produces a reducer that can be used to reduce the results at
    /// the end.
    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer);

    /// Convert the consumer into a folder that can consume items
    /// sequentially, eventually producing a final result.
    fn into_folder(self) -> Self::Folder;

}

pub trait Folder<Item> {
    type Result;

    /// Consume next item and return new sequential state.
    fn consume(self, item: Item) -> Self;

    /// Finish consuming items, produce final result.
    fn complete(self) -> Self::Result;
}

pub trait Reducer<Result> {
    /// Reduce two final results into one; this is executed after a
    /// split.
    fn reduce(self, left: Result, right: Result) -> Result;
}

/// A stateless consumer can be freely copied.
pub trait UnindexedConsumer<ITEM>: Consumer<ITEM> {
    fn split_off(&self) -> Self;
    fn to_reducer(&self) -> Self::Reducer;
}

/// A splitter controls the policy for splitting into smaller work items.
trait Splitter: Send + Sized {
    fn needs_split(&self) -> bool;
    fn split_off(self) -> (Self, Self);
}

/// Classic cost-splitting uses weights to split until below a threshold.
#[derive(Clone, Copy)]
struct CostSplitter(f64);

impl Splitter for CostSplitter {
    #[inline]
    fn needs_split(&self) -> bool {
        self.0 > THRESHOLD
    }

    #[inline]
    fn split_off(mut self) -> (Self, Self) {
        self.0 /= 2.0;
        (self, self)
    }
}

/// Thief-splitting is an adaptive policy that starts by splitting into enough
/// jobs for every worker thread, and then resets itself whenever a job is
/// actually stolen into a different thread.
#[derive(Clone, Copy)]
struct ThiefSplitter {
    origin: usize,
    splits: usize,
}

impl ThiefSplitter {
    #[inline]
    fn id() -> usize {
        // The actual `ID` value is irrelevant.  We're just using its TLS
        // address as a unique thread key, faster than a real thread-id call.
        thread_local!{ static ID: bool = false; }
        ID.with(|id| id as *const bool as usize )
    }

    #[inline]
    fn new() -> ThiefSplitter {
        ThiefSplitter {
            origin: ThiefSplitter::id(),
            splits: get_registry().num_threads(),
        }
    }
}

impl Splitter for ThiefSplitter {
    #[inline]
    fn needs_split(&self) -> bool {
        self.splits > 0 || self.origin != ThiefSplitter::id()
    }

    #[inline]
    fn split_off(mut self) -> (Self, Self) {
        let id = ThiefSplitter::id();
        if self.origin != id {
            self.origin = id;
            self.splits = get_registry().num_threads();
        } else {
            self.splits /= 2;
        }
        (self, self)
    }
}

pub fn bridge<PAR_ITER,C>(mut par_iter: PAR_ITER,
                             consumer: C)
                             -> C::Result
    where PAR_ITER: IndexedParallelIterator, C: Consumer<PAR_ITER::Item>
{
    let len = par_iter.len();
    return par_iter.with_producer(Callback { len: len,
                                             consumer: consumer, });

    struct Callback<C> {
        len: usize,
        consumer: C,
    }

    impl<C, ITEM> ProducerCallback<ITEM> for Callback<C>
        where C: Consumer<ITEM>
    {
        type Output = C::Result;
        fn callback<P>(mut self, mut producer: P) -> C::Result
            where P: Producer<Item=ITEM>
        {
            if producer.weighted() || self.consumer.weighted() {
                let producer_cost = producer.cost(self.len);
                let cost = self.consumer.cost(producer_cost);
                let splitter = CostSplitter(cost);
                bridge_producer_consumer(self.len, splitter, producer, self.consumer)
            } else {
                let splitter = ThiefSplitter::new();
                bridge_producer_consumer(self.len, splitter, producer, self.consumer)
            }
        }
    }
}

fn bridge_producer_consumer<S,P,C>(len: usize,
                                   splitter: S,
                                   producer: P,
                                   consumer: C)
                                   -> C::Result
    where S: Splitter, P: Producer, C: Consumer<P::Item>
{
    if len > 1 && splitter.needs_split() {
        let mid = len / 2;
        let (left_splitter, right_splitter) = splitter.split_off();
        let (left_producer, right_producer) = producer.split_at(mid);
        let (left_consumer, right_consumer, reducer) = consumer.split_at(mid);
        let (left_result, right_result) =
            join(|| bridge_producer_consumer(mid, left_splitter,
                                             left_producer, left_consumer),
                 || bridge_producer_consumer(len - mid, right_splitter,
                                             right_producer, right_consumer));
        reducer.reduce(left_result, right_result)
    } else {
        let mut folder = consumer.into_folder();
        for item in producer {
            folder = folder.consume(item);
        }
        folder.complete()
    }
}

/// Utility type for consumers that don't need a "reduce" step. Just
/// reduces unit to unit.
pub struct NoopReducer;

impl Reducer<()> for NoopReducer {
    fn reduce(self, _left: (), _right: ()) { }
}
