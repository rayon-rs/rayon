#![cfg(test)]

extern crate compiletest_rs as compiletest;

use configuration::*;
use std::error::Error;
use std::sync::{Arc, Barrier, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
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
    let n_threads = 16;
    let n_called = Arc::new(AtomicUsize::new(0));
    // Wait for all the threads in the pool plus the one running tests.
    let barrier = Arc::new(Barrier::new(n_threads + 1));

    let b = barrier.clone();
    let nc = n_called.clone();
    let start_handler: StartHandler = Arc::new(move |_| {
        nc.fetch_add(1, Ordering::SeqCst);
        b.wait();
    });

    let conf = Configuration::new()
        .set_num_threads(n_threads)
        .set_start_handler(start_handler);
    let _ = ThreadPool::new(conf).unwrap();

    // Wait for all the threads to have been scheduled to run.
    barrier.wait();

    // The handler must have been called on every started thread.
    assert_eq!(n_called.load(Ordering::SeqCst), n_threads);
}

#[test]
fn exit_callback_called() {
    let (tx, rx) = channel();
    let tx = Mutex::new(tx);

    let exit_handler: ExitHandler = Arc::new(move |index| {
        let tx = tx.lock().unwrap();
        tx.send(index).unwrap();
    });

    let n_threads = 16;
    let conf = Configuration::new()
        .set_num_threads(n_threads)
        .set_exit_handler(exit_handler);

    {
        let _ = ThreadPool::new(conf).unwrap();
    }

    // Drain the message queue.
    let mut exited = 0;
    for msg in rx {
        let mask = 1 << msg;
        assert!(exited & mask == 0);

        exited |= mask;
        if exited == 0xffff {
            break;
        }
    }
}
