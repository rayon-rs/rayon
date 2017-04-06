#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use Configuration;
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

    {
        // once we exit this block, thread-pool will be dropped
        let thread_pool = ThreadPool::new(Configuration::new().num_threads(22)).unwrap();
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
        join(|| join_a_lot(n - 1), || join_a_lot(n - 1));
    }
}

#[test]
fn sleeper_stop() {
    use std::{thread, time};

    let registry;

    {
        // once we exit this block, thread-pool will be dropped
        let thread_pool = ThreadPool::new(Configuration::new().num_threads(22)).unwrap();
        registry = thread_pool.registry.clone();

        // Give time for at least some of the thread pool to fall asleep.
        thread::sleep(time::Duration::from_secs(1));
    }

    // once thread-pool is dropped, registry should terminate, which
    // should lead to worker threads stopping
    registry.wait_until_stopped();
}

/// Create a start/exit handler that increments an atomic counter.
fn count_handler() -> (Arc<AtomicUsize>, Box<::StartHandler>) {
    let count = Arc::new(AtomicUsize::new(0));
    (count.clone(), Box::new(move |_| { count.fetch_add(1, Ordering::SeqCst); }))
}

/// Immediately unwrap a counter, asserting that it's no longer shared.
fn unwrap_counter(counter: Arc<AtomicUsize>) -> usize {
    Arc::try_unwrap(counter)
        .expect("Counter is still shared!")
        .into_inner()
}

/// Wait until a counter is no longer shared, then return its value.
fn wait_for_counter(mut counter: Arc<AtomicUsize>) -> usize {
    use std::{thread, time};

    for _ in 0..60 {
        counter = match Arc::try_unwrap(counter) {
            Ok(counter) => return counter.into_inner(),
            Err(counter) => {
                thread::sleep(time::Duration::from_secs(1));
                counter
            }
        };
    }

    // That's too long -- do it now!
    unwrap_counter(counter)
}

/// For some reason, macs do not error out here.
#[cfg_attr(target_os="macos", ignore)]
#[test]
fn failed_thread_stack() {
    let (start_count, start_handler) = count_handler();
    let (exit_count, exit_handler) = count_handler();
    let config = Configuration::new()
        .stack_size(::std::usize::MAX)
        .start_handler(move |i| start_handler(i))
        .exit_handler(move |i| exit_handler(i));

    let pool = ThreadPool::new(config);
    assert!(pool.is_err(), "thread stack should have failed!");

    // With an impossible stack, we don't expect to have seen any threads,
    // nor should there be any remaining references to the counters.
    assert_eq!(0, unwrap_counter(start_count));
    assert_eq!(0, unwrap_counter(exit_count));
}

#[test]
fn panic_thread_name() {
    let (start_count, start_handler) = count_handler();
    let (exit_count, exit_handler) = count_handler();
    let config = Configuration::new()
        .num_threads(10)
        .start_handler(move |i| start_handler(i))
        .exit_handler(move |i| exit_handler(i))
        .thread_name(|i| {
                         if i >= 5 {
                             panic!();
                         }
                         format!("panic_thread_name#{}", i)
                     });

    let pool = unwind::halt_unwinding(|| ThreadPool::new(config));
    assert!(pool.is_err(), "thread-name panic should propagate!");

    // Assuming they're created in order, threads 0 through 4 should have
    // been started already, and then terminated by the panic.
    assert_eq!(5, wait_for_counter(start_count));
    assert_eq!(5, wait_for_counter(exit_count));
}
