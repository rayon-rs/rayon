//! Internal traits and functions used to implement parallel
//! iteration. These should be considered highly unstable: users of
//! parallel iterators should not need to interact with them directly.
//! See `README.md` for a high-level overview.

use join;
use super::IndexedParallelIterator;
use super::len::*;

/// A producer which will produce a fixed number of items N. This is
/// not queryable through the API; the consumer is expected to track
/// it.
pub trait Producer: Send {
    type Item;
    type Shared: Sync;

    /// Cost to produce `len` items.
    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64;

    /// Split into two producers; one produces items `0..index`, the
    /// other `index..N`. Index must be less than `N`.
    unsafe fn split_at(self, index: usize) -> (Self, Self);

    /// Unless a panic occurs, expects to be called *exactly N times*.
    unsafe fn produce(&mut self, shared: &Self::Shared) -> Self::Item;
}

/// A consumer which consumes items within the lifetime `'consume`.
pub trait Consumer<'consume>: Send {
    type Item: 'consume;
    type Shared: Sync + 'consume;
    type SeqState;
    type Result: Send;

    /// If it costs `producer_cost` to produce the items we will
    /// consume, returns cost adjusted to account for consuming them.
    fn cost(&mut self, shared: &Self::Shared, producer_cost: f64) -> f64;

    /// Divide the consumer into two consumers, one processing items
    /// `0..index` and one processing items from `index..`.
    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self);

    /// Start processing items. This can return some sequential state
    /// that will be threaded through as items are consumed.
    unsafe fn start(&mut self, shared: &Self::Shared) -> Self::SeqState;

    /// Consume next item and return new sequential state.
    unsafe fn consume(&mut self,
                      shared: &Self::Shared,
                      state: Self::SeqState,
                      item: Self::Item)
                      -> Self::SeqState;

    /// Finish consuming items, produce final result.
    unsafe fn complete(self, shared: &Self::Shared, state: Self::SeqState) -> Self::Result;

    /// Reduce two final results into one; this is executed after a
    /// split.
    unsafe fn reduce(shared: &Self::Shared,
                     left: Self::Result,
                     right: Self::Result)
                     -> Self::Result;
}

/// A stateless consumer can be freely copied.
pub trait UnindexedConsumer<'c>: Consumer<'c> {
    fn split(&self) -> Self;
}

pub fn bridge<'c,PAR_ITER,C>(mut par_iter: PAR_ITER,
                             mut consumer: C,
                             consumer_shared: &'c C::Shared)
                             -> C::Result
    where PAR_ITER: IndexedParallelIterator, C: Consumer<'c, Item=PAR_ITER::Item>
{
    let len = par_iter.len();
    let (mut producer, producer_shared) = par_iter.into_producer();
    let producer_cost = producer.cost(&producer_shared, len);
    let cost = consumer.cost(consumer_shared, producer_cost);
    bridge_producer_consumer(len, cost, producer, &producer_shared, consumer, consumer_shared)
}

fn bridge_producer_consumer<'c,P,C>(len: usize,
                                    cost: f64,
                                    mut producer: P,
                                    producer_shared: &P::Shared,
                                    mut consumer: C,
                                    consumer_shared: &'c C::Shared)
                                    -> C::Result
    where P: Producer, C: Consumer<'c, Item=P::Item>
{
    unsafe { // asserting that we call the `op` methods in correct pattern
        if len > 1 && cost > THRESHOLD {
            let mid = len / 2;
            let (left_producer, right_producer) = producer.split_at(mid);
            let (left_consumer, right_consumer) = consumer.split_at(consumer_shared, mid);
            let (left_result, right_result) =
                join(|| bridge_producer_consumer(mid, cost / 2.0,
                                                 left_producer, producer_shared,
                                                 left_consumer, consumer_shared),
                     || bridge_producer_consumer(len - mid, cost / 2.0,
                                                 right_producer, producer_shared,
                                                 right_consumer, consumer_shared));
            C::reduce(consumer_shared, left_result, right_result)
        } else {
            let mut consumer_state = consumer.start(consumer_shared);
            for _ in 0..len {
                let item = producer.produce(producer_shared);
                consumer_state = consumer.consume(consumer_shared, consumer_state, item);
            }
            consumer.complete(consumer_shared, consumer_state)
        }
    }
}
