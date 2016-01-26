use join;
use super::PullParallelIterator;
use super::len::*;

/// The trait for types representing the internal *state* during
/// parallelization. This basically represents a group of tasks
/// to be done.
///
/// Note that this trait is declared as an **unsafe trait**. That
/// means that the trait is unsafe to implement. The reason is that
/// other bits of code, such as the `collect` routine on
/// `ParallelIterator`, rely on the `len` and `for_each` functions
/// being accurate and correct. For example, if the `len` function
/// reports that it will produce N items, then `for_each` *must*
/// produce `N` items or else the resulting vector will contain
/// uninitialized memory.
///
/// This trait is not really intended to be implemented outside of the
/// Rayon crate at this time. The precise safety requirements are kind
/// of ill-documented for this reason (i.e., they are ill-understood).
pub unsafe trait ParallelIteratorState: Sized {
    type Item;
    type Shared: Sync;

    /// Returns an estimate of how much work is to be done.
    ///
    /// # Safety note
    ///
    /// If sparse is false, then `maximal_len` must be precisely
    /// correct.
    fn len(&mut self, shared: &Self::Shared) -> ParallelLen;

    /// Split this state into two other states, ideally of roughly
    /// equal size.
    fn split_at(self, index: usize) -> (Self, Self);

    /// Extract the next item from this iterator state. Once this
    /// method is called, sequential iteration has begun, and the
    /// other methods will no longer be called.
    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item>;
}

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

pub trait Consumer: Send {
    type Item;
    type Shared: Sync;
    type SeqState;
    type Result: Send;

    /// If it costs `producer_cost` to produce the items we will
    /// consume, returns cost adjusted to account for consuming them.
    fn cost(&mut self, shared: &Self::Shared, producer_cost: f64) -> f64;

    /// Divide the consumer into two consumers, one processing items
    /// `0..index` and one processing items `index..`.
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

pub fn bridge<PAR_ITER,C>(mut par_iter: PAR_ITER,
                          mut consumer: C,
                          consumer_shared: &C::Shared)
                          -> C::Result
    where PAR_ITER: PullParallelIterator, C: Consumer<Item=PAR_ITER::Item>
{
    let len = par_iter.len();
    let (mut producer, producer_shared) = par_iter.into_producer();
    let producer_cost = producer.cost(&producer_shared, len);
    let cost = consumer.cost(consumer_shared, producer_cost);
    bridge_producer_consumer(len, cost, producer, &producer_shared, consumer, consumer_shared)
}

fn bridge_producer_consumer<P,C>(len: usize,
                                 cost: f64,
                                 mut producer: P,
                                 producer_shared: &P::Shared,
                                 mut consumer: C,
                                 consumer_shared: &C::Shared)
                                 -> C::Result
    where P: Producer, C: Consumer<Item=P::Item>
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
