//! Internal traits and functions used to implement parallel
//! iteration. These should be considered highly unstable: users of
//! parallel iterators should not need to interact with them directly.
//! See `README.md` for a high-level overview.

use join;
use std::cmp::max;
use super::IndexedParallelIterator;

pub trait ProducerCallback<ITEM> {
    type Output;
    fn callback<P>(self, producer: P) -> Self::Output
        where P: Producer<Item=ITEM>;
}

/// A producer which will produce a fixed number of items N. This is
/// not queryable through the API; the consumer is expected to track
/// it.
pub trait Producer: IntoIterator + Send + Sized {
    /// Split into two producers; one produces items `0..index`, the
    /// other `index..N`. Index must be less than `N`.
    fn split_at(self, index: usize) -> (Self, Self);
}

/// A consumer which consumes items that are fed to it.
pub trait Consumer<Item>: Send + Sized {
    type Folder: Folder<Item, Result=Self::Result>;
    type Reducer: Reducer<Self::Result>;
    type Result: Send;

    /// Threshold below which it doesn't make sense to split. If this
    /// were e.g. 100, then if we had a thread that could produce 110
    /// items for us, it would prefer to split to two threads of 55,
    /// but at that point it wouldn't try to split again. For most
    /// parallel iterators, defaults to 1 (so keep splitting as much
    /// as you can), but can be controlled by the user.
    fn sequential_threshold(&self) -> usize;

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
        fn callback<P>(self, producer: P) -> C::Result
            where P: Producer<Item=ITEM>
        {
            let consumer_threshold = max(self.consumer.sequential_threshold(), 1);
            bridge_producer_consumer(self.len, consumer_threshold, producer, self.consumer)
        }
    }
}

fn bridge_producer_consumer<P,C>(len: usize,
                                 threshold: usize,
                                 producer: P,
                                 consumer: C)
                                 -> C::Result
    where P: Producer, C: Consumer<P::Item>
{
    debug_assert!(threshold >= 1);
    if len > threshold {
        let mid = len / 2;
        let (left_producer, right_producer) = producer.split_at(mid);
        let (left_consumer, right_consumer, reducer) = consumer.split_at(mid);
        let (left_result, right_result) =
            join(|| bridge_producer_consumer(mid, threshold,
                                             left_producer, left_consumer),
                 || bridge_producer_consumer(len - mid, threshold,
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
