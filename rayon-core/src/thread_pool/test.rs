#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use crate::join;
use crate::thread_pool::ThreadPool;

#[allow(deprecated)]
use crate::Configuration;
use crate::ThreadPoolBuilder;

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate() {
    let thread_pool = ThreadPoolBuilder::new().build().unwrap();
    thread_pool.install(|| {
        panic!("Hello, world!");
    });
}

#[test]
fn workers_stop() {
    let registry;

    {
        // once we exit this block, thread-pool will be dropped
        let thread_pool = ThreadPoolBuilder::new().num_threads(22).build().unwrap();
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
        let thread_pool = ThreadPoolBuilder::new().num_threads(22).build().unwrap();
        registry = thread_pool.registry.clone();

        // Give time for at least some of the thread pool to fall asleep.
        thread::sleep(time::Duration::from_secs(1));
    }

    // once thread-pool is dropped, registry should terminate, which
    // should lead to worker threads stopping
    registry.wait_until_stopped();
}

/// Creates a start/exit handler that increments an atomic counter.
fn count_handler() -> (Arc<AtomicUsize>, impl Fn(usize)) {
    let count = Arc::new(AtomicUsize::new(0));
    (count.clone(), move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    })
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

    // That's too long!
    panic!("Counter is still shared!");
}

#[test]
fn failed_thread_stack() {
    // Note: we first tried to force failure with a `usize::MAX` stack, but
    // macOS and Windows weren't fazed, or at least didn't fail the way we want.
    // They work with `isize::MAX`, but 32-bit platforms may feasibly allocate a
    // 2GB stack, so it might not fail until the second thread.
    let stack_size = ::std::isize::MAX as usize;

    let (start_count, start_handler) = count_handler();
    let (exit_count, exit_handler) = count_handler();
    let builder = ThreadPoolBuilder::new()
        .num_threads(10)
        .stack_size(stack_size)
        .start_handler(start_handler)
        .exit_handler(exit_handler);

    let pool = builder.build();
    assert!(pool.is_err(), "thread stack should have failed!");

    // With such a huge stack, 64-bit will probably fail on the first thread;
    // 32-bit might manage the first 2GB, but certainly fail the second.
    let start_count = wait_for_counter(start_count);
    assert!(start_count <= 1);
    assert_eq!(start_count, wait_for_counter(exit_count));
}

#[test]
fn panic_thread_name() {
    let (start_count, start_handler) = count_handler();
    let (exit_count, exit_handler) = count_handler();
    let builder = ThreadPoolBuilder::new()
        .num_threads(10)
        .start_handler(start_handler)
        .exit_handler(exit_handler)
        .thread_name(|i| {
            if i >= 5 {
                panic!();
            }
            format!("panic_thread_name#{}", i)
        });

    let pool = crate::unwind::halt_unwinding(|| builder.build());
    assert!(pool.is_err(), "thread-name panic should propagate!");

    // Assuming they're created in order, threads 0 through 4 should have
    // been started already, and then terminated by the panic.
    assert_eq!(5, wait_for_counter(start_count));
    assert_eq!(5, wait_for_counter(exit_count));
}

#[test]
fn self_install() {
    let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    // If the inner `install` blocks, then nothing will actually run it!
    assert!(pool.install(|| pool.install(|| true)));
}

#[test]
fn mutual_install() {
    let pool1 = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let pool2 = ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    let ok = pool1.install(|| {
        // This creates a dependency from `pool1` -> `pool2`
        pool2.install(|| {
            // This creates a dependency from `pool2` -> `pool1`
            pool1.install(|| {
                // If they blocked on inter-pool installs, there would be no
                // threads left to run this!
                true
            })
        })
    });
    assert!(ok);
}

#[test]
fn mutual_install_sleepy() {
    use std::{thread, time};

    let pool1 = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let pool2 = ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    let ok = pool1.install(|| {
        // This creates a dependency from `pool1` -> `pool2`
        pool2.install(|| {
            // Give `pool1` time to fall asleep.
            thread::sleep(time::Duration::from_secs(1));

            // This creates a dependency from `pool2` -> `pool1`
            pool1.install(|| {
                // Give `pool2` time to fall asleep.
                thread::sleep(time::Duration::from_secs(1));

                // If they blocked on inter-pool installs, there would be no
                // threads left to run this!
                true
            })
        })
    });
    assert!(ok);
}

#[test]
#[allow(deprecated)]
fn check_thread_pool_new() {
    let pool = ThreadPool::new(Configuration::new().num_threads(22)).unwrap();
    assert_eq!(pool.current_num_threads(), 22);
}

macro_rules! test_scope_order {
    ($scope:ident => $spawn:ident) => {{
        let builder = ThreadPoolBuilder::new().num_threads(1);
        let pool = builder.build().unwrap();
        pool.install(|| {
            let vec = Mutex::new(vec![]);
            pool.$scope(|scope| {
                let vec = &vec;
                for i in 0..10 {
                    scope.$spawn(move |_| {
                        vec.lock().unwrap().push(i);
                    });
                }
            });
            vec.into_inner().unwrap()
        })
    }};
}

#[test]
fn scope_lifo_order() {
    let vec = test_scope_order!(scope => spawn);
    let expected: Vec<i32> = (0..10).rev().collect(); // LIFO -> reversed
    assert_eq!(vec, expected);
}

#[test]
fn scope_fifo_order() {
    let vec = test_scope_order!(scope_fifo => spawn_fifo);
    let expected: Vec<i32> = (0..10).collect(); // FIFO -> natural order
    assert_eq!(vec, expected);
}

macro_rules! test_spawn_order {
    ($spawn:ident) => {{
        let builder = ThreadPoolBuilder::new().num_threads(1);
        let pool = &builder.build().unwrap();
        let (tx, rx) = channel();
        pool.install(move || {
            for i in 0..10 {
                let tx = tx.clone();
                pool.$spawn(move || {
                    tx.send(i).unwrap();
                });
            }
        });
        rx.iter().collect::<Vec<i32>>()
    }};
}

#[test]
fn spawn_lifo_order() {
    let vec = test_spawn_order!(spawn);
    let expected: Vec<i32> = (0..10).rev().collect(); // LIFO -> reversed
    assert_eq!(vec, expected);
}

#[test]
fn spawn_fifo_order() {
    let vec = test_spawn_order!(spawn_fifo);
    let expected: Vec<i32> = (0..10).collect(); // FIFO -> natural order
    assert_eq!(vec, expected);
}
