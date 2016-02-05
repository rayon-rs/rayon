//! Internal traits and functions used to implement parallel
//! iteration. These should be considered highly unstable: users of
//! parallel iterators should not need to interact with them directly.
//! See `README.md` for a high-level overview.

use join;
use super::IndexedParallelIterator;
use super::len::*;
use super::util::PhantomType;

pub trait ProducerCallback<ITEM> {
    type Output;
    fn callback<'p, P>(self, producer: P, shared: &'p P::Shared) -> Self::Output
        where P: Producer<'p, Item=ITEM>;
}

/// A producer which will produce a fixed number of items N. This is
/// not queryable through the API; the consumer is expected to track
/// it.
pub trait Producer<'produce>: Send {
    type Item;
    type Shared: Sync + 'produce;

    /// Cost to produce `len` items, where `len` must be `N`.
    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64;

    /// Split into two producers; one produces items `0..index`, the
    /// other `index..N`. Index must be less than `N`.
    unsafe fn split_at(self, index: usize) -> (Self, Self);

    /// Unless a panic occurs, expects to be called *exactly N times*.
    unsafe fn produce(&mut self, shared: &Self::Shared) -> Self::Item;
}

/// A consumer which consumes items that are fed to it.
pub trait Consumer: Send {
    type Item;
    type SeqState;
    type Result: Send;

    /// If it costs `producer_cost` to produce the items we will
    /// consume, returns cost adjusted to account for consuming them.
    fn cost(&mut self, producer_cost: f64) -> f64;

    /// Divide the consumer into two consumers, one processing items
    /// `0..index` and one processing items from `index..`.
    unsafe fn split_at(self, index: usize) -> (Self, Self);

    /// Start processing items. This can return some sequential state
    /// that will be threaded through as items are consumed.
    unsafe fn start(&mut self) -> Self::SeqState;

    /// Consume next item and return new sequential state.
    unsafe fn consume(&mut self,
                      state: Self::SeqState,
                      item: Self::Item)
                      -> Self::SeqState;

    /// Finish consuming items, produce final result.
    unsafe fn complete(self, state: Self::SeqState) -> Self::Result;

    /// Reduce two final results into one; this is executed after a
    /// split.
    unsafe fn reduce(left: Self::Result, right: Self::Result) -> Self::Result;
}

/// A stateless consumer can be freely copied.
pub trait UnindexedConsumer: Consumer {
    fn split(&self) -> Self;
}

pub fn bridge<'c,PAR_ITER,C>(mut par_iter: PAR_ITER,
                             consumer: C)
                             -> C::Result
    where PAR_ITER: IndexedParallelIterator, C: Consumer<Item=PAR_ITER::Item>
{
    let len = par_iter.len();
    return par_iter.with_producer(Callback { len: len,
                                             consumer: consumer,
                                             phantoms: PhantomType::new() });

    struct Callback<'c, C> {
        len: usize,
        consumer: C,
        phantoms: PhantomType<&'c ()>
    }

    impl<'c, C> ProducerCallback<C::Item> for Callback<'c, C>
        where C: Consumer
    {
        type Output = C::Result;
        fn callback<'p, P>(mut self,
                           mut producer: P,
                           producer_shared: &'p P::Shared)
                           -> C::Result
            where P: Producer<'p, Item=C::Item>
        {
            let producer_cost = producer.cost(&producer_shared, self.len);
            let cost = self.consumer.cost(producer_cost);
            bridge_producer_consumer(self.len, cost,
                                     producer, producer_shared,
                                     self.consumer)
        }
    }
}

fn bridge_producer_consumer<'p,'c,P,C>(len: usize,
                                       cost: f64,
                                       mut producer: P,
                                       producer_shared: &'p P::Shared,
                                       mut consumer: C)
                                       -> C::Result
    where P: Producer<'p>, C: Consumer<Item=P::Item>
{
    unsafe { // asserting that we call the `op` methods in correct pattern
        if len > 1 && cost > THRESHOLD {
            let mid = len / 2;
            let (left_producer, right_producer) = producer.split_at(mid);
            let (left_consumer, right_consumer) = consumer.split_at(mid);
            let (left_result, right_result) =
                join(|| bridge_producer_consumer(mid, cost / 2.0,
                                                 left_producer, producer_shared,
                                                 left_consumer),
                     || bridge_producer_consumer(len - mid, cost / 2.0,
                                                 right_producer, producer_shared,
                                                 right_consumer));
            C::reduce(left_result, right_result)
        } else {
            let mut consumer_state = consumer.start();
            for _ in 0..len {
                let item = producer.produce(producer_shared);
                consumer_state = consumer.consume(consumer_state, item);
            }
            consumer.complete(consumer_state)
        }
    }
}
