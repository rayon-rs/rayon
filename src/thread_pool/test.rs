#![cfg(test)]

use configuration::Configuration;
use prelude::*;
use super::ThreadPool;

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate() {
    let thread_pool = ThreadPool::new(Configuration::new()).unwrap();
    thread_pool.install(|| {
        panic!("Hello, world!");
    });
}

#[test]
fn workers_stop() {
    let registry;

    { // once we exit this block, thread-pool will be dropped
        let thread_pool = ThreadPool::new(Configuration::new().set_num_threads(22)).unwrap();
        registry = thread_pool.install(|| {
            // do some work on these threads
            let s1 = (0..10*1024).into_par_iter().sum();
            let s2 = (0..10*1024).sum();
            assert_eq!(s1, s2);

            thread_pool.registry.clone()
        });
        assert_eq!(registry.num_threads(), 22);
    }

    // once thread-pool is dropped, registry should terminate, which
    // should lead to worker threads stopping
    registry.wait_until_stopped();
}

#[test]
fn sleeper_stop() {
    use std::{thread, time};

    let registry;

    { // once we exit this block, thread-pool will be dropped
        let thread_pool = ThreadPool::new(Configuration::new().set_num_threads(22)).unwrap();
        registry = thread_pool.registry.clone();

        // Give time for at least some of the thread pool to fall asleep.
        thread::sleep(time::Duration::from_secs(1));
    }

    // once thread-pool is dropped, registry should terminate, which
    // should lead to worker threads stopping
    registry.wait_until_stopped();
}
