#![cfg(test)]

extern crate compiletest_rs as compiletest;

use configuration::*;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use thread_pool::*;

#[test]
fn worker_thread_index() {
    let pool = ThreadPool::new(Configuration::new().set_num_threads(22)).unwrap();
    assert_eq!(pool.num_threads(), 22);
    assert_eq!(pool.current_thread_index(), None);
    let index = pool.install(|| pool.current_thread_index().unwrap());
    assert!(index < 22);
}

#[test]
fn start_callback_called() {
    let mut n_called = Arc::new(AtomicUsize::new(0));

    let conf = Configuration::new()
        .set_num_threads(16)
        .set_start_handler(Arc::new(|| {
            n_called.fetch_add(1, Ordering::SeqCst);
        }));
    {
        let pool = ThreadPool::new(conf).unwrap();

        pool.install(|| 0);
        pool.install(|| 1);
    }

    // We must have started at least one thread.
    assert!(n_called.load(Ordering::SeqCst) > 0);
}
