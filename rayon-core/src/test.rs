#![cfg(test)]

use Configuration;
use std::sync::{Arc, Barrier};
use std::sync::atomic::{AtomicUsize, Ordering};
use thread_pool::*;

#[test]
fn worker_thread_index() {
    let pool = ThreadPool::new(Configuration::new().num_threads(22)).unwrap();
    assert_eq!(pool.current_num_threads(), 22);
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
    let start_handler = move |_| {
        nc.fetch_add(1, Ordering::SeqCst);
        b.wait();
    };

    let conf = Configuration::new()
        .num_threads(n_threads)
        .start_handler(start_handler);
    let _ = ThreadPool::new(conf).unwrap();

    // Wait for all the threads to have been scheduled to run.
    barrier.wait();

    // The handler must have been called on every started thread.
    assert_eq!(n_called.load(Ordering::SeqCst), n_threads);
}

#[test]
fn exit_callback_called() {
    let n_threads = 16;
    let n_called = Arc::new(AtomicUsize::new(0));
    // Wait for all the threads in the pool plus the one running tests.
    let barrier = Arc::new(Barrier::new(n_threads + 1));

    let b = barrier.clone();
    let nc = n_called.clone();
    let exit_handler = move |_| {
        nc.fetch_add(1, Ordering::SeqCst);
        b.wait();
    };

    let conf = Configuration::new()
        .num_threads(n_threads)
        .exit_handler(exit_handler);
    {
        let _ = ThreadPool::new(conf).unwrap();
        // Drop the pool so it stops the running threads.
    }

    // Wait for all the threads to have been scheduled to run.
    barrier.wait();

    // The handler must have been called on every exiting thread.
    assert_eq!(n_called.load(Ordering::SeqCst), n_threads);
}

#[test]
fn handler_panics_handled_correctly() {
    let n_threads = 16;
    let n_called = Arc::new(AtomicUsize::new(0));
    // Wait for all the threads in the pool plus the one running tests.
    let start_barrier = Arc::new(Barrier::new(n_threads + 1));
    let exit_barrier = Arc::new(Barrier::new(n_threads + 1));

    let start_handler = move |_| {
        panic!("ensure panic handler is called when starting");
    };
    let exit_handler = move |_| {
        panic!("ensure panic handler is called when exiting");
    };

    let sb = start_barrier.clone();
    let eb = exit_barrier.clone();
    let nc = n_called.clone();
    let panic_handler = move |_| {
        let val = nc.fetch_add(1, Ordering::SeqCst);
        if val < n_threads {
            sb.wait();
        } else {
            eb.wait();
        }
    };

    let conf = Configuration::new()
        .num_threads(n_threads)
        .start_handler(start_handler)
        .exit_handler(exit_handler)
        .panic_handler(panic_handler);
    {
        let _ = ThreadPool::new(conf).unwrap();

        // Wait for all the threads to start, panic in the start handler,
        // and been taken care of by the panic handler.
        start_barrier.wait();

        // Drop the pool so it stops the running threads.
    }

    // Wait for all the threads to exit, panic in the exit handler,
    // and been taken care of by the panic handler.
    exit_barrier.wait();

    // The panic handler must have been called twice on every thread.
    assert_eq!(n_called.load(Ordering::SeqCst), 2 * n_threads);
}
