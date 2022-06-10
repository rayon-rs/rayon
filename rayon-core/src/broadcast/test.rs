#![cfg(test)]

use crate::ThreadPoolBuilder;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn broadcast_global() {
    let v = crate::broadcast(|ctx| ctx.index());
    assert!(v.into_iter().eq(0..crate::current_num_threads()));
}

#[test]
fn broadcast_pool() {
    let pool = ThreadPoolBuilder::new().num_threads(7).build().unwrap();
    let v = pool.broadcast(|ctx| ctx.index());
    assert!(v.into_iter().eq(0..7));
}

#[test]
fn broadcast_self() {
    let pool = ThreadPoolBuilder::new().num_threads(7).build().unwrap();
    let v = pool.install(|| crate::broadcast(|ctx| ctx.index()));
    assert!(v.into_iter().eq(0..7));
}

#[test]
fn broadcast_mutual() {
    let count = AtomicUsize::new(0);
    let pool1 = ThreadPoolBuilder::new().num_threads(3).build().unwrap();
    let pool2 = ThreadPoolBuilder::new().num_threads(7).build().unwrap();
    pool1.install(|| {
        pool2.broadcast(|_| {
            pool1.broadcast(|_| {
                count.fetch_add(1, Ordering::Relaxed);
            })
        })
    });
    assert_eq!(count.into_inner(), 3 * 7);
}

#[test]
fn broadcast_mutual_sleepy() {
    use std::{thread, time};

    let count = AtomicUsize::new(0);
    let pool1 = ThreadPoolBuilder::new().num_threads(3).build().unwrap();
    let pool2 = ThreadPoolBuilder::new().num_threads(7).build().unwrap();
    pool1.install(|| {
        thread::sleep(time::Duration::from_secs(1));
        pool2.broadcast(|_| {
            thread::sleep(time::Duration::from_secs(1));
            pool1.broadcast(|_| {
                thread::sleep(time::Duration::from_millis(100));
                count.fetch_add(1, Ordering::Relaxed);
            })
        })
    });
    assert_eq!(count.into_inner(), 3 * 7);
}

#[test]
fn broadcast_panic_one() {
    let count = AtomicUsize::new(0);
    let pool = ThreadPoolBuilder::new().num_threads(7).build().unwrap();
    let result = crate::unwind::halt_unwinding(|| {
        pool.broadcast(|ctx| {
            count.fetch_add(1, Ordering::Relaxed);
            if ctx.index() == 3 {
                panic!("Hello, world!");
            }
        })
    });
    assert_eq!(count.into_inner(), 7);
    assert!(result.is_err(), "broadcast panic should propagate!");
}

#[test]
fn broadcast_panic_many() {
    let count = AtomicUsize::new(0);
    let pool = ThreadPoolBuilder::new().num_threads(7).build().unwrap();
    let result = crate::unwind::halt_unwinding(|| {
        pool.broadcast(|ctx| {
            count.fetch_add(1, Ordering::Relaxed);
            if ctx.index() % 2 == 0 {
                panic!("Hello, world!");
            }
        })
    });
    assert_eq!(count.into_inner(), 7);
    assert!(result.is_err(), "broadcast panic should propagate!");
}
