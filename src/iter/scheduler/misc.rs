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
