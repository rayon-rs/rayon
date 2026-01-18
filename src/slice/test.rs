#![cfg(test)]

use crate::prelude::*;
use rand::distr::Uniform;
use rand::seq::IndexedRandom;
use rand::{rng, Rng};
use std::cmp::Ordering::{Equal, Greater, Less};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

macro_rules! sort {
    ($f:ident, $name:ident) => {
        #[test]
        fn $name() {
            let rng = &mut rng();

            for len in (0..25).chain(500..501) {
                for &modulus in &[5, 10, 100] {
                    let dist = Uniform::new(0, modulus).unwrap();
                    for _ in 0..100 {
                        let v: Vec<i32> = rng.sample_iter(&dist).take(len).collect();

                        // Test sort using `<` operator.
                        let mut tmp = v.clone();
                        tmp.$f(|a, b| a.cmp(b));
                        assert!(tmp.windows(2).all(|w| w[0] <= w[1]));

                        // Test sort using `>` operator.
                        let mut tmp = v.clone();
                        tmp.$f(|a, b| b.cmp(a));
                        assert!(tmp.windows(2).all(|w| w[0] >= w[1]));
                    }
                }
            }

            // Test sort with many duplicates.
            for &len in &[1_000, 10_000, 100_000] {
                for &modulus in &[5, 10, 100, 10_000] {
                    let dist = Uniform::new(0, modulus).unwrap();
                    let mut v: Vec<i32> = rng.sample_iter(&dist).take(len).collect();

                    v.$f(|a, b| a.cmp(b));
                    assert!(v.windows(2).all(|w| w[0] <= w[1]));
                }
            }

            // Test sort with many pre-sorted runs.
            for &len in &[1_000, 10_000, 100_000] {
                let len_dist = Uniform::new(0, len).unwrap();
                for &modulus in &[5, 10, 1000, 50_000] {
                    let dist = Uniform::new(0, modulus).unwrap();
                    let mut v: Vec<i32> = rng.sample_iter(&dist).take(len).collect();

                    v.sort();
                    v.reverse();

                    for _ in 0..5 {
                        let a = rng.sample(&len_dist);
                        let b = rng.sample(&len_dist);
                        if a < b {
                            v[a..b].reverse();
                        } else {
                            v.swap(a, b);
                        }
                    }

                    v.$f(|a, b| a.cmp(b));
                    assert!(v.windows(2).all(|w| w[0] <= w[1]));
                }
            }

            // Sort using a completely random comparison function.
            // This will reorder the elements *somehow*, but won't panic.
            let mut v: Vec<_> = (0..100).collect();
            v.$f(|_, _| *[Less, Equal, Greater].choose(&mut rand::rng()).unwrap());
            v.$f(|a, b| a.cmp(b));
            for i in 0..v.len() {
                assert_eq!(v[i], i);
            }

            // Should not panic.
            [0i32; 0].$f(|a, b| a.cmp(b));
            [(); 10].$f(|a, b| a.cmp(b));
            [(); 100].$f(|a, b| a.cmp(b));

            let mut v = [0xDEAD_BEEFu64];
            v.$f(|a, b| a.cmp(b));
            assert!(v == [0xDEAD_BEEF]);
        }
    };
}

sort!(par_sort_by, test_par_sort);
sort!(par_sort_unstable_by, test_par_sort_unstable);

#[test]
fn test_par_sort_stability() {
    for len in (2..25).chain(500..510).chain(50_000..50_010) {
        for _ in 0..10 {
            let mut counts = [0; 10];

            // Create a vector like [(6, 1), (5, 1), (6, 2), ...],
            // where the first item of each tuple is random, but
            // the second item represents which occurrence of that
            // number this element is, i.e. the second elements
            // will occur in sorted order.
            let mut rng = rng();
            let mut v: Vec<_> = (0..len)
                .map(|_| {
                    let n: usize = rng.random_range(0..10);
                    counts[n] += 1;
                    (n, counts[n])
                })
                .collect();

            // Only sort on the first element, so an unstable sort
            // may mix up the counts.
            v.par_sort_by(|&(a, _), &(b, _)| a.cmp(&b));

            // This comparison includes the count (the second item
            // of the tuple), so elements with equal first items
            // will need to be ordered with increasing
            // counts... i.e. exactly asserting that this sort is
            // stable.
            assert!(v.windows(2).all(|w| w[0] <= w[1]));
        }
    }
}

#[test]
fn test_par_chunks_exact_remainder() {
    let v: &[i32] = &[0, 1, 2, 3, 4];
    let c = v.par_chunks_exact(2);
    assert_eq!(c.remainder(), &[4]);
    assert_eq!(c.len(), 2);
}

#[test]
fn test_par_chunks_exact_mut_remainder() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let mut c = v.par_chunks_exact_mut(2);
    assert_eq!(c.remainder(), &[4]);
    assert_eq!(c.len(), 2);
    assert_eq!(c.into_remainder(), &[4]);

    let mut c = v.par_chunks_exact_mut(2);
    assert_eq!(c.take_remainder(), &[4]);
    assert_eq!(c.take_remainder(), &[]);
    assert_eq!(c.len(), 2);
}

#[test]
fn test_par_rchunks_exact_remainder() {
    let v: &[i32] = &[0, 1, 2, 3, 4];
    let c = v.par_rchunks_exact(2);
    assert_eq!(c.remainder(), &[0]);
    assert_eq!(c.len(), 2);
}

#[test]
fn test_par_rchunks_exact_mut_remainder() {
    let v: &mut [i32] = &mut [0, 1, 2, 3, 4];
    let mut c = v.par_rchunks_exact_mut(2);
    assert_eq!(c.remainder(), &[0]);
    assert_eq!(c.len(), 2);
    assert_eq!(c.into_remainder(), &[0]);

    let mut c = v.par_rchunks_exact_mut(2);
    assert_eq!(c.take_remainder(), &[0]);
    assert_eq!(c.take_remainder(), &[]);
    assert_eq!(c.len(), 2);
}

#[test]
fn slice_chunk_by() {
    let v: Vec<_> = (0..1000).collect();
    assert_eq!(v[..0].par_chunk_by(|_, _| todo!()).count(), 0);
    assert_eq!(v[..1].par_chunk_by(|_, _| todo!()).count(), 1);
    assert_eq!(v[..2].par_chunk_by(|_, _| true).count(), 1);
    assert_eq!(v[..2].par_chunk_by(|_, _| false).count(), 2);

    let count = AtomicUsize::new(0);
    let par: Vec<_> = v
        .par_chunk_by(|x, y| {
            count.fetch_add(1, Relaxed);
            (x % 10 < 3) == (y % 10 < 3)
        })
        .collect();
    assert_eq!(count.into_inner(), v.len() - 1);

    let seq: Vec<_> = v.chunk_by(|x, y| (x % 10 < 3) == (y % 10 < 3)).collect();
    assert_eq!(par, seq);
}

#[test]
fn slice_chunk_by_mut() {
    let mut v: Vec<_> = (0..1000).collect();
    assert_eq!(v[..0].par_chunk_by_mut(|_, _| todo!()).count(), 0);
    assert_eq!(v[..1].par_chunk_by_mut(|_, _| todo!()).count(), 1);
    assert_eq!(v[..2].par_chunk_by_mut(|_, _| true).count(), 1);
    assert_eq!(v[..2].par_chunk_by_mut(|_, _| false).count(), 2);

    let mut v2 = v.clone();
    let count = AtomicUsize::new(0);
    let par: Vec<_> = v
        .par_chunk_by_mut(|x, y| {
            count.fetch_add(1, Relaxed);
            (x % 10 < 3) == (y % 10 < 3)
        })
        .collect();
    assert_eq!(count.into_inner(), v2.len() - 1);

    let seq: Vec<_> = v2
        .chunk_by_mut(|x, y| (x % 10 < 3) == (y % 10 < 3))
        .collect();
    assert_eq!(par, seq);
}

#[test]
fn test_par_median() {
    let v = vec![5, 1, 9, 3, 7, 2, 8, 4, 6];
    let median = v.par_median().unwrap();
    assert_eq!(median, 5);

    let v: Vec<i32> = (0..10).collect();
    let median = v.par_median().unwrap();
    assert_eq!(median, 5);

    let v: Vec<i32> = (0..11).collect();
    let median = v.par_median().unwrap();
    assert_eq!(median, 5);

    assert!([0i32; 0].par_median().is_none());
}

#[test]
fn test_par_k_smallest() {
    let v = vec![5, 1, 9, 3, 7, 2, 8, 4, 6];
    let third_smallest = v.par_k_smallest(2).unwrap();
    assert_eq!(third_smallest, 3);

    let v: Vec<i32> = (0..10).collect();
    let smallest = v.par_k_smallest(0).unwrap();
    assert_eq!(smallest, 0);

    let largest = v.par_k_smallest(9).unwrap();
    assert_eq!(largest, 9);

    assert!(vec![1, 2, 3].par_k_smallest(5).is_none());
}

#[test]
fn test_par_k_largest() {
    let v = vec![5, 1, 9, 3, 7, 2, 8, 4, 6];
    let third_largest = v.par_k_largest(2).unwrap();
    assert_eq!(third_largest, 7);

    let v: Vec<i32> = (0..10).collect();
    let largest = v.par_k_largest(0).unwrap();
    assert_eq!(largest, 9);

    let smallest = v.par_k_largest(9).unwrap();
    assert_eq!(smallest, 0);

    assert!(vec![1, 2, 3].par_k_largest(5).is_none());
}

#[test]
fn test_par_median_mut() {
    let mut v = vec![5, 1, 9, 3, 7, 2, 8, 4, 6];
    let median = v.par_median_mut().unwrap();
    assert_eq!(median, 5);

    let mut v: Vec<i32> = (0..10).collect();
    let median = v.par_median_mut().unwrap();
    assert_eq!(median, 5);

    let mut v: Vec<i32> = (0..11).collect();
    let median = v.par_median_mut().unwrap();
    assert_eq!(median, 5);

    assert!([0i32; 0].par_median_mut().is_none());
}

#[test]
fn test_par_k_smallest_mut() {
    let mut v = vec![5, 1, 9, 3, 7, 2, 8, 4, 6];
    let third_smallest = v.par_k_smallest_mut(2).unwrap();
    assert_eq!(third_smallest, 3);

    let mut v: Vec<i32> = (0..10).collect();
    let smallest = v.par_k_smallest_mut(0).unwrap();
    assert_eq!(smallest, 0);

    let largest = v.par_k_smallest_mut(9).unwrap();
    assert_eq!(largest, 9);

    assert!(vec![1, 2, 3].par_k_smallest_mut(5).is_none());
}

#[test]
fn test_par_k_largest_mut() {
    let mut v = vec![5, 1, 9, 3, 7, 2, 8, 4, 6];
    let third_largest = v.par_k_largest_mut(2).unwrap();
    assert_eq!(third_largest, 7);

    let mut v: Vec<i32> = (0..10).collect();
    let largest = v.par_k_largest_mut(0).unwrap();
    assert_eq!(largest, 9);

    let smallest = v.par_k_largest_mut(9).unwrap();
    assert_eq!(smallest, 0);

    assert!(vec![1, 2, 3].par_k_largest_mut(5).is_none());
}
