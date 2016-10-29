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

    /// Hint whether this `Consumer` would like to stop processing
    /// further items, e.g. if a search has been completed.
    fn full(&self) -> bool { false }
}

pub trait Folder<Item> {
    type Result;

    /// Consume next item and return new sequential state.
    fn consume(self, item: Item) -> Self;

    /// Finish consuming items, produce final result.
    fn complete(self) -> Self::Result;

    /// Hint whether this `Folder` would like to stop processing
    /// further items, e.g. if a search has been completed.
    fn full(&self) -> bool { false }
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
#[derive(Clone, Copy)]
enum Splitter {
    /// Classic cost-splitting uses weights to split until below a threshold.
    Cost(f64),

    /// Thief-splitting is an adaptive policy that starts by splitting into
    /// enough jobs for every worker thread, and then resets itself whenever a
    /// job is actually stolen into a different thread.
    Thief(usize, usize),
}

impl Splitter {
    #[inline]
    fn thief_id() -> usize {
        // The actual `ID` value is irrelevant.  We're just using its TLS
        // address as a unique thread key, faster than a real thread-id call.
        thread_local!{ static ID: bool = false }
        ID.with(|id| id as *const bool as usize )
    }

    #[inline]
    fn new_thief() -> Splitter {
        Splitter::Thief(Splitter::thief_id(), get_registry().num_threads())
    }

    #[inline]
    fn try(&mut self) -> bool {
        match *self {
            Splitter::Cost(ref mut cost) => {
                if *cost > THRESHOLD {
                    *cost /= 2.0;
                    true
                } else { false }
            },

            Splitter::Thief(ref mut origin, ref mut splits) => {
                let id = Splitter::thief_id();
                if *origin != id {
                    *origin = id;
                    *splits = get_registry().num_threads();
                    true
                } else if *splits > 0 {
                    *splits /= 2;
                    true
                } else { false }
            }
        }
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
            let splitter = if producer.weighted() || self.consumer.weighted() {
                let producer_cost = producer.cost(self.len);
                let cost = self.consumer.cost(producer_cost);
                Splitter::Cost(cost)
            } else {
                Splitter::new_thief()
            };
            bridge_producer_consumer(self.len, splitter, producer, self.consumer)
        }
    }
}

fn bridge_producer_consumer<P,C>(len: usize,
                                 mut splitter: Splitter,
                                 producer: P,
                                 consumer: C)
                                 -> C::Result
    where P: Producer, C: Consumer<P::Item>
{
    if consumer.full() {
        consumer.into_folder().complete()
    } else if len > 1 && splitter.try() {
        let mid = len / 2;
        let (left_producer, right_producer) = producer.split_at(mid);
        let (left_consumer, right_consumer, reducer) = consumer.split_at(mid);
        let (left_result, right_result) =
            join(move || bridge_producer_consumer(mid, splitter,
                                                  left_producer, left_consumer),
                 move || bridge_producer_consumer(len - mid, splitter,
                                                  right_producer, right_consumer));
        reducer.reduce(left_result, right_result)
    } else {
        let mut folder = consumer.into_folder();
        for item in producer {
            folder = folder.consume(item);
            if folder.full() { break }
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
