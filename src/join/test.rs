//! Tests for the join code.

#![cfg(test)]

use ::*;
use rand::{Rng, SeedableRng, XorShiftRng};

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

    let result = initialize(Configuration::new());

    match result {
        Ok(_) => {
            quick_sort(&mut data);

            let mut sorted_data = data.clone();
            sorted_data.sort();

            assert_eq!(data, sorted_data);
        }
        Err(e) => panic!("expected InitOk, but got {:?}", e),
    }
}

#[test]
fn sort_in_pool() {
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let mut data: Vec<_> = (0..12 * 1024).map(|_| rng.next_u32()).collect();

    let result = ThreadPool::new(Configuration::new());

    match result {
        Ok(pool) => {
            pool.install(|| {
                quick_sort(&mut data);
            });

            let mut sorted_data = data.clone();
            sorted_data.sort();

            assert_eq!(data, sorted_data);
        }
        Err(_) => panic!("expected Ok() but got Err()"),
    }
}

