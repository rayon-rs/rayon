use super::plumbing::*;
use super::*;

/// `ByBlocks` is a parallel iterator that consumes itself as a sequence
/// of parallel blocks.
///
/// This struct is created by the [`by_blocks()`] method on [`IndexedParallelIterator`]
///
/// [`by_blocks()`]: trait.IndexedParallelIterator.html#method.by_blocks
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct ByBlocks<I, S> {
    base: I,
    sizes: S,
}

impl<I, S> ByBlocks<I, S>
where
    I: IndexedParallelIterator,
    S: IntoIterator<Item = usize>,
{
    /// Creates a new `ByBlocks` iterator.
    pub(super) fn new(base: I, sizes: S) -> Self {
        ByBlocks { base, sizes }
    }
}

struct BlocksCallback<S, C> {
    sizes: S,
    consumer: C,
    len: usize,
}

impl<T, S, C> ProducerCallback<T> for BlocksCallback<S, C>
where
    C: UnindexedConsumer<T>,
    S: Iterator<Item = usize>,
{
    type Output = C::Result;
    fn callback<P: Producer<Item = T>>(mut self, mut producer: P) -> Self::Output {
        let mut remaining_len = self.len;
        let mut consumer = self.consumer;
        // we need a local variable for the accumulated results
        // we call the reducer's identity by splitting at 0
        let (left_consumer, right_consumer, _) = consumer.split_at(0);
        let mut leftmost_res = left_consumer.into_folder().complete();
        consumer = right_consumer;

        // now we loop on each block size
        while remaining_len > 0 && !consumer.full() {
            // we compute the next block's size
            let size = self.sizes.next().unwrap_or(std::usize::MAX);
            let capped_size = remaining_len.min(size);
            remaining_len -= capped_size;

            // split the producer
            let (left_producer, right_producer) = producer.split_at(capped_size);
            producer = right_producer;
            // split the consumer
            let (left_consumer, right_consumer, _) = consumer.split_at(capped_size);
            consumer = right_consumer;
            leftmost_res = consumer.to_reducer().reduce(
                leftmost_res,
                bridge_producer_consumer(capped_size, left_producer, left_consumer),
            );
        }
        leftmost_res
    }
}

impl<I, S> ParallelIterator for ByBlocks<I, S>
where
    I: IndexedParallelIterator,
    S: Iterator<Item = usize> + Send,
{
    type Item = I::Item;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let len = self.base.len();
        let callback = BlocksCallback {
            consumer,
            sizes: self.sizes,
            len,
        };
        self.base.with_producer(callback)
    }
}
