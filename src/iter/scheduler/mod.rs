//! This module is dedicated to custom scheduler API and useful schedulers.

use super::*;

pub mod misc;

pub use misc::*;

/// Scheduler for Indexed Parallel Iterator
pub trait Scheduler: Send {
    /// Consume one Indexed Producer and one Indexed Consumer. Split the work acordingly.
    fn bridge<P, C, T>(&mut self, len: usize, producer: P, consumer: C) -> C::Result
    where
        P: Producer<Item = T>,
        C: Consumer<T>;
}

/// Scheduler for Unindexed Parallel Iterator
pub trait UnindexedScheduler: Send {
    /// Consume one Unindexed Producer and one Unindexed Consumer. Split the work acordingly.
    fn bridge_unindexed<P, C, T>(&mut self, producer: P, consumer: C) -> C::Result
    where
        P: UnindexedProducer<Item = T>,
        C: UnindexedConsumer<T>;
}

/// `WithScheduler` is an iterator that enclose one Indexed Scheduler.
#[derive(Debug)]
pub struct WithScheduler<I: IndexedParallelIterator, S> {
    base: I,
    scheduler: S,
}

/// `WithUnindexedScheduler` is an iterator that enclose one Unindexed Scheduler.
#[derive(Debug)]
pub struct WithUnindexedScheduler<I: ParallelIterator, S> {
    base: I,
    scheduler: S,
}

impl<I: IndexedParallelIterator, S> WithScheduler<I, S> {
    /// Create a new `WithUnindexedScheduler` iterator.
    pub(super) fn new(base: I, scheduler: S) -> Self {
        WithScheduler { base, scheduler }
    }
}

impl<I: ParallelIterator, S> WithUnindexedScheduler<I, S> {
    /// Create a new `WithUnindexedScheduler` iterator.
    pub(super) fn new(base: I, scheduler: S) -> Self {
        WithUnindexedScheduler { base, scheduler }
    }
}

macro_rules! no_link {
    ($l:literal) => {
        {
            extern "C" {
                #[link_name = $l]
                fn trigger() -> !;
            }
            unsafe { trigger() }
        }
    };
}

impl<I: IndexedParallelIterator, S: Scheduler> ParallelIterator for WithScheduler<I, S> {
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        IndexedParallelIterator::drive(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        self.base.opt_len()
    }
}

impl<I: IndexedParallelIterator, S: Scheduler> IndexedParallelIterator for WithScheduler<I, S> {
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        let len = self.base.len();
        return self.base.with_producer(Callback {
            len: len,
            scheduler: self.scheduler,
            consumer: consumer,
        });

        struct Callback<S, C> {
            len: usize,
            scheduler: S,
            consumer: C,
        }

        impl<S, C, I> ProducerCallback<I> for Callback<S, C>
        where
            S: Scheduler,
            C: Consumer<I>,
        {
            type Output = C::Result;
            fn callback<P>(mut self, producer: P) -> C::Result
            where
                P: Producer<Item = I>,
            {
                self.scheduler.bridge(self.len, producer, self.consumer)
            }
        }
    }
    fn len(&self) -> usize {
        self.base.len()
    }
    fn with_producer<CB>(self, _callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        no_link!( "\n\nERROR[rayon]: After `with_scheduler`, the iterator should not be used as producer.\nFor example, `Zip` works in producer mode, so `with_scheduler` should not happen before any `zip`.\nFor more information about producer and customer mode, please refer to https://github.com/rayon-rs/rayon/blob/master/src/iter/plumbing/README.md\n")
    }
}

impl<I: ParallelIterator, S: UnindexedScheduler> ParallelIterator for WithUnindexedScheduler<I, S> {
    type Item = I::Item;

    fn drive_unindexed<C>(self, _consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        todo!()
    }

    fn opt_len(&self) -> Option<usize> {
        self.base.opt_len()
    }
}
