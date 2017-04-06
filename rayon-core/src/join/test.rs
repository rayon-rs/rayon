//! Tests for the join code.

use Configuration;
use join::*;
use rand::{Rng, SeedableRng, XorShiftRng};
use thread_pool::*;
use unwind;

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

#[test]
fn sort() {
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let mut data: Vec<_> = (0..6 * 1024).map(|_| rng.next_u32()).collect();
    let mut sorted_data = data.clone();
    sorted_data.sort();
    quick_sort(&mut data);
    assert_eq!(data, sorted_data);
}

#[test]
fn sort_in_pool() {
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let mut data: Vec<_> = (0..12 * 1024).map(|_| rng.next_u32()).collect();

    let pool = ThreadPool::new(Configuration::new()).unwrap();
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
