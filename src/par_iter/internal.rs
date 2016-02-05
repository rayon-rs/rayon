//! Internal traits and functions used to implement parallel
//! iteration. These should be considered highly unstable: users of
//! parallel iterators should not need to interact with them directly.
//! See `README.md` for a high-level overview.

use join;
use super::IndexedParallelIterator;
use super::len::*;

pub trait ProducerCallback<ITEM> {
    type Output;
    fn callback<'p, P>(self, producer: P) -> Self::Output
        where P: Producer<Item=ITEM>;
}

/// A producer which will produce a fixed number of items N. This is
/// not queryable through the API; the consumer is expected to track
/// it.
pub trait Producer: Send {
    type Item;

    /// Cost to produce `len` items, where `len` must be `N`.
    fn cost(&mut self, len: usize) -> f64;

    /// Split into two producers; one produces items `0..index`, the
    /// other `index..N`. Index must be less than `N`.
    fn split_at(self, index: usize) -> (Self, Self);

    /// Unless a panic occurs, expects to be called *exactly N times*.
    fn produce(&mut self) -> Self::Item;
}

/// A consumer which consumes items that are fed to it.
pub trait Consumer<Item>: Send {
    type Folder: Folder<Item, Result=Self::Result>;
    type Reducer: Reducer<Self::Result>;
    type Result: Send;

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
    fn fold(self) -> Self::Folder;

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
    fn split(&self) -> Self;
    fn reducer(&self) -> Self::Reducer;
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
            let producer_cost = producer.cost(self.len);
            let cost = self.consumer.cost(producer_cost);
            bridge_producer_consumer(self.len, cost, producer, self.consumer)
        }
    }
}

fn bridge_producer_consumer<P,C>(len: usize,
                                 cost: f64,
                                 mut producer: P,
                                 consumer: C)
                                 -> C::Result
    where P: Producer, C: Consumer<P::Item>
{
    if len > 1 && cost > THRESHOLD {
        let mid = len / 2;
        let (left_producer, right_producer) = producer.split_at(mid);
        let (left_consumer, right_consumer, reducer) = consumer.split_at(mid);
        let (left_result, right_result) =
            join(|| bridge_producer_consumer(mid, cost / 2.0,
                                             left_producer, left_consumer),
                 || bridge_producer_consumer(len - mid, cost / 2.0,
                                             right_producer, right_consumer));
        reducer.reduce(left_result, right_result)
    } else {
        let mut folder = consumer.fold();
        for _ in 0..len {
            let item = producer.produce();
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

