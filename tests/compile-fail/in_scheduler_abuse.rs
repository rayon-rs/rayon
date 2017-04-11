extern crate rayon;

use rayon::prelude::*;
use rayon::iter::internal::*;

struct DummyScheduler;

impl<'a> Scheduler for &'a DummyScheduler {
    fn execute_indexed<P, C>(self,
                             len: usize,
                             producer: P,
                             consumer: C)
                             -> C::Result
        where P: Producer,
              C: Consumer<P::Item>
    {
        DefaultScheduler.execute_indexed(len, producer, consumer)
    }

    fn execute_unindexed<P, C>(self,
                               producer: P,
                               consumer: C)
                               -> C::Result
        where P: UnindexedProducer,
              C: UnindexedConsumer<P::Item>
    {
        DefaultScheduler.execute_unindexed(producer, consumer)
    }
}

fn zip_lhs() {
    (0..512).into_par_iter()
            .in_scheduler(&DummyScheduler)
            .zip(0..512) //~ ERROR E0271
            .for_each(|_| ());
}

fn zip_rhs() {
    (0..512).into_par_iter()
            .in_scheduler(&DummyScheduler)
            .zip((0..512).into_par_iter().in_scheduler(&DummyScheduler)) //~ ERROR E0271
    //~^ ERROR E0271
            .for_each(|_| ());
}

fn chain_lhs() {
    (0..512).into_par_iter()
            .in_scheduler(&DummyScheduler)
            .chain(0..512); //~ ERROR E0271
}

fn chain_rhs() {
    (0..512).into_par_iter()
            .in_scheduler(&DummyScheduler)
            .chain((0..512).into_par_iter().in_scheduler(&DummyScheduler)); //~ ERROR E0271
    //~^ ERROR E0271
    //~| ERROR type mismatch
}

fn dummy_and_dummy() {
    (0..512).into_par_iter()
            .map(|i| i * 2)
            .in_scheduler(&DummyScheduler)
            .in_scheduler(&DummyScheduler); //~ ERROR type mismatch
}

fn default_and_dummy() {
    (0..512).into_par_iter()
            .map(|i| i * 2)
            .in_scheduler(DefaultScheduler)
            .in_scheduler(&DummyScheduler); //~ ERROR type mismatch
}

fn default_twice() {
    (0..512).into_par_iter()
            .map(|i| i * 2)
            .in_scheduler(DefaultScheduler)
            .in_scheduler(DefaultScheduler); //~ ERROR type mismatch
}

fn main() { }
