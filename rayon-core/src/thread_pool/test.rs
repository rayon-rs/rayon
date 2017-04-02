#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use configuration::Configuration;
use join;
use super::ThreadPool;
use unwind;

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
            join_a_lot(22);

            thread_pool.registry.clone()
        });
        assert_eq!(registry.num_threads(), 22);
    }

    // once thread-pool is dropped, registry should terminate, which
    // should lead to worker threads stopping
    registry.wait_until_stopped();
}

fn join_a_lot(n: usize) {
    if n > 0 {
        join(|| join_a_lot(n-1), || join_a_lot(n-1));
    }
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

fn count_handler() -> (Arc<AtomicUsize>, ::StartHandler) {
    let count = Arc::new(AtomicUsize::new(0));
    (count.clone(), Arc::new(move |_| { count.fetch_add(1, Ordering::SeqCst); }))
}

#[test]
fn failed_thread_stack() {
    use std::{thread, time};

    let (start_count, start_handler) = count_handler();
    let (exit_count, exit_handler) = count_handler();
    let config = Configuration::new()
        .set_stack_size(::std::usize::MAX)
        .set_start_handler(start_handler)
        .set_exit_handler(exit_handler);

    let pool = ThreadPool::new(config);
    assert!(pool.is_err(), "thread stack should have failed!");

    // Give time for the internal cleanup to terminate.
    thread::sleep(time::Duration::from_secs(1));

    // With an impossible stack, we don't expect to see any threads.
    assert_eq!(0, start_count.load(Ordering::SeqCst));
    assert_eq!(0, exit_count.load(Ordering::SeqCst));
}

#[test]
fn panic_thread_name() {
    use std::{thread, time};

    let (start_count, start_handler) = count_handler();
    let (exit_count, exit_handler) = count_handler();
    let config = Configuration::new()
        .set_num_threads(10)
        .set_start_handler(start_handler)
        .set_exit_handler(exit_handler)
        .set_thread_name(|i| {
                             if i >= 5 {
                                 panic!();
                             }
                             format!("panic_thread_name#{}", i)
                         });

    let pool = unwind::halt_unwinding(|| ThreadPool::new(config));
    assert!(pool.is_err(), "thread-name panic should propagate!");

    // Give time for the internal cleanup to terminate.
    thread::sleep(time::Duration::from_secs(1));

    // Assuming they're created in order, threads 0 through 4 should have
    // been started already, and then terminated by the panic.
    assert_eq!(5, start_count.load(Ordering::SeqCst));
    assert_eq!(5, exit_count.load(Ordering::SeqCst));
}
