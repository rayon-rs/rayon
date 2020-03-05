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
