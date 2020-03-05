//! This module contains useful schedulers.
use super::*;

/// Default Scheduler.
/// When used as Indexed Scheduler, Thief-splitting will be used.
/// When used as Unindexed Scheduler, tasks will be divided to minimum piece.
#[derive(Debug, Clone, Default)]
pub struct DefaultScheduler;

impl Scheduler for DefaultScheduler {
    fn bridge<P, C, T>(&mut self, len: usize, producer: P, consumer: C) -> C::Result
    where
        P: Producer<Item = T>,
        C: Consumer<T>,
    {
        bridge_producer_consumer(len, producer, consumer)
    }
}

impl UnindexedScheduler for DefaultScheduler {
    fn bridge_unindexed<P, C, T>(&mut self, producer: P, consumer: C) -> C::Result
    where
        P: UnindexedProducer<Item = T>,
        C: UnindexedConsumer<T>,
    {
        bridge_unindexed(producer, consumer)
    }
}

/// Dummy Sequential Scheduler.
/// No parallel is used at all.
#[derive(Debug, Clone, Default)]
pub struct SequentialScheduler;

impl Scheduler for SequentialScheduler {
    fn bridge<P, C, T>(&mut self, _len: usize, producer: P, consumer: C) -> C::Result
    where
        P: Producer<Item = T>,
        C: Consumer<T>,
    {
        producer.fold_with(consumer.into_folder()).complete()
    }
}

impl UnindexedScheduler for SequentialScheduler {
    fn bridge_unindexed<P, C, T>(&mut self, producer: P, consumer: C) -> C::Result
    where
        P: UnindexedProducer<Item = T>,
        C: UnindexedConsumer<T>,
    {
        producer.fold_with(consumer.into_folder()).complete()
    }
}

fn static_partition_bridge<P, C, T>(positions: &[usize], producer: P, consumer: C) -> C::Result
where
    P: Producer<Item = T>,
    C: Consumer<T>,
{
    fn helper<P, C, T>(positions: &[usize], bias: usize, producer: P, consumer: C) -> C::Result
    where
        P: Producer<Item = T>,
        C: Consumer<T>,
    {
        if consumer.full() {
            consumer.into_folder().complete()
        } else if positions.len() > 0 {
            let mid_index = positions.len() / 2;
            let position = positions[mid_index];

            let (left_producer, right_producer) = producer.split_at(position - bias);
            let (left_consumer, right_consumer, reducer) = consumer.split_at(position - bias);

            use crate::join;
            let (left_result, right_result) = join(
                || helper(&positions[0..mid_index], bias, left_producer, left_consumer),
                || {
                    helper(
                        &positions[mid_index + 1..],
                        position,
                        right_producer,
                        right_consumer,
                    )
                },
            );
            reducer.reduce(left_result, right_result)
        } else {
            producer.fold_with(consumer.into_folder()).complete()
        }
    }
    helper(positions, 0, producer, consumer)
}

/// Fixed length scheduler.
/// Every tasks assigned to a thread will contain a fixed number of items,
/// except for the last task which will possibly contain less.
/// The parameter in `with_min_len` and `with_max_len` will be ignored.
#[derive(Debug, Clone, Default)]
pub struct FixedLengthScheduler {
    fixed_length: usize,
}

impl FixedLengthScheduler {
    /// Create fixed length scheduler with assigned length. Length must be greater than or equal to 1.
    pub fn new(fixed_length: usize) -> Self {
        if fixed_length == 0 {
            panic!("Length must be greater than or equal to 1.")
        };
        Self { fixed_length }
    }
}

impl Scheduler for FixedLengthScheduler {
    fn bridge<P, C, T>(&mut self, len: usize, producer: P, consumer: C) -> C::Result
    where
        P: Producer<Item = T>,
        C: Consumer<T>,
    {
        let positions: Vec<_> = (0..len).step_by(self.fixed_length).skip(1).collect();
        static_partition_bridge(&positions, producer, consumer)
    }
}

/// Static split scheduler.
/// Given a chunk size, this scheduler will divide all items evenly based on their
/// length to create `current_num_threads()` number of tasks.
/// The length of each task should be multiple of the chunk size, except for the last task.
#[derive(Debug, Clone, Default)]
pub struct StaticScheduler {
    chunk_size: usize,
}

impl StaticScheduler {
    /// Create static split scheduler with default chunk size 1.
    pub fn new() -> Self {
        Self { chunk_size: 1 }
    }
    /// Create static split scheduler with assigned chunk size. Chunk size must be greater than or equal to 1.
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        if chunk_size == 0 {
            panic!("Chunk size must be greater than or equal to 1.")
        };
        Self {
            chunk_size: chunk_size,
        }
    }
}

impl Scheduler for StaticScheduler {
    fn bridge<P, C, T>(&mut self, len: usize, producer: P, consumer: C) -> C::Result
    where
        P: Producer<Item = T>,
        C: Consumer<T>,
    {
        use crate::current_num_threads;
        let num_threads = current_num_threads();
        let full_chunks = len / self.chunk_size;
        let positions: Vec<_> = (1..num_threads)
            .map(|i| (i * full_chunks) / num_threads * self.chunk_size)
            .collect();

        static_partition_bridge(&positions, producer, consumer)
    }
}
