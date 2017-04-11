#![cfg(test)]

use iter::internal::*;
use prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

extern crate thread_id;

struct SequentialScheduler {
    indexed: AtomicUsize,
    unindexed: AtomicUsize,
}

impl<'a> Scheduler for &'a SequentialScheduler {
    fn execute_indexed<P, C>(self,
                             len: usize,
                             producer: P,
                             consumer: C)
                             -> C::Result
        where P: Producer,
              C: Consumer<P::Item>
    {
        self.indexed.fetch_add(1, Ordering::SeqCst);
        consumer.into_folder().consume_iter(producer.into_iter()).complete()
    }

    fn execute_unindexed<P, C>(self,
                               producer: P,
                               consumer: C)
                               -> C::Result
        where P: UnindexedProducer,
              C: UnindexedConsumer<P::Item>
    {
        self.unindexed.fetch_add(1, Ordering::SeqCst);
        producer.fold_with(consumer.into_folder()).complete()
    }
}

#[test]
fn seq_scheduler() {
    let scheduler = SequentialScheduler {
        indexed: AtomicUsize::new(0),
        unindexed: AtomicUsize::new(0),
    };

    let this_id = thread_id::get();

    (0..512).into_par_iter()
            .map(|i| i * 2)
            .in_scheduler(&scheduler)
            .for_each(|_| assert_eq!(thread_id::get(), this_id));

    assert!(scheduler.indexed.load(Ordering::SeqCst) == 1);
    assert!(scheduler.unindexed.load(Ordering::SeqCst) == 0);
}
