//! Tests for the join code.

use join::*;
use rand::distributions::Standard;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use unwind;
use ThreadPoolBuilder;

fn quick_sort<T: PartialOrd + Send>(v: &mut [T]) {
    if v.len() <= 1 {
        return;
    }

    let mid = partition(v);
    let (lo, hi) = v.split_at_mut(mid);
    join(|| quick_sort(lo), || quick_sort(hi));
}

fn partition<T: PartialOrd + Send>(v: &mut [T]) -> usize {
    let pivot = v.len() - 1;
    let mut i = 0;
    for j in 0..pivot {
        if v[j] <= v[pivot] {
            v.swap(i, j);
            i += 1;
        }
    }
    v.swap(i, pivot);
    i
}

fn seeded_rng() -> XorShiftRng {
    let mut seed = <XorShiftRng as SeedableRng>::Seed::default();
    (0..).zip(seed.as_mut()).for_each(|(i, x)| *x = i);
    XorShiftRng::from_seed(seed)
}

#[test]
fn sort() {
    let rng = seeded_rng();
    let mut data: Vec<u32> = rng.sample_iter(&Standard).take(6 * 1024).collect();
    let mut sorted_data = data.clone();
    sorted_data.sort();
    quick_sort(&mut data);
    assert_eq!(data, sorted_data);
}

#[test]
fn sort_in_pool() {
    let rng = seeded_rng();
    let mut data: Vec<u32> = rng.sample_iter(&Standard).take(12 * 1024).collect();

    let pool = ThreadPoolBuilder::new().build().unwrap();
    let mut sorted_data = data.clone();
    sorted_data.sort();
    pool.install(|| quick_sort(&mut data));
    assert_eq!(data, sorted_data);
}

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate_a() {
    join(|| panic!("Hello, world!"), || ());
}

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate_b() {
    join(|| (), || panic!("Hello, world!"));
}

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate_both() {
    join(|| panic!("Hello, world!"), || panic!("Goodbye, world!"));
}

#[test]
fn panic_b_still_executes() {
    let mut x = false;
    match unwind::halt_unwinding(|| join(|| panic!("Hello, world!"), || x = true)) {
        Ok(_) => panic!("failed to propagate panic from closure A,"),
        Err(_) => assert!(x, "closure b failed to execute"),
    }
}

#[test]
fn join_context_both() {
    // If we're not in a pool, both should be marked stolen as they're injected.
    let (a_migrated, b_migrated) = join_context(|a| a.migrated(), |b| b.migrated());
    assert!(a_migrated);
    assert!(b_migrated);
}

#[test]
fn join_context_neither() {
    // If we're already in a 1-thread pool, neither job should be stolen.
    let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let (a_migrated, b_migrated) =
        pool.install(|| join_context(|a| a.migrated(), |b| b.migrated()));
    assert!(!a_migrated);
    assert!(!b_migrated);
}

#[test]
fn join_context_second() {
    use std::sync::Barrier;

    // If we're already in a 2-thread pool, the second job should be stolen.
    let barrier = Barrier::new(2);
    let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
    let (a_migrated, b_migrated) = pool.install(|| {
        join_context(
            |a| {
                barrier.wait();
                a.migrated()
            },
            |b| {
                barrier.wait();
                b.migrated()
            },
        )
    });
    assert!(!a_migrated);
    assert!(b_migrated);
}
