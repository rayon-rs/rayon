//! Benchmark Factorial N! = 1×2×⋯×N

use num::{BigUint, One};
use rayon::prelude::*;
use std::ops::Mul;

const N: u32 = 9999;

/// Compute the Factorial using a plain iterator.
fn factorial(n: u32) -> BigUint {
    (1..=n).map(BigUint::from).fold(BigUint::one(), Mul::mul)
}

#[bench]
/// Benchmark the Factorial using a plain iterator.
fn factorial_iterator(b: &mut test::Bencher) {
    let f = factorial(N);
    b.iter(|| assert_eq!(factorial(test::black_box(N)), f));
}

#[bench]
/// Compute the Factorial using rayon::par_iter.
fn factorial_par_iter(b: &mut test::Bencher) {
    fn fact(n: u32) -> BigUint {
        (1..n + 1)
            .into_par_iter()
            .map(BigUint::from)
            .reduce_with(Mul::mul)
            .unwrap()
    }

    let f = factorial(N);
    b.iter(|| assert_eq!(fact(test::black_box(N)), f));
}

#[bench]
/// Compute the Factorial using rayon::fold_with.
fn factorial_fold_with(b: &mut test::Bencher) {
    fn fact(n: u32) -> BigUint {
        (1..n + 1)
            .into_par_iter()
            .with_min_len(64) // for fair comparison with factorial_fold_chunks_with()
            .fold_with(BigUint::from(1_u32), |acc, x| acc.mul(x))
            .reduce_with(Mul::mul)
            .unwrap()
    }

    let f = factorial(N);
    b.iter(|| assert_eq!(fact(test::black_box(N)), f));
}

#[bench]
/// Compute the Factorial using rayon::fold_chunks_with.
fn factorial_fold_chunks_with(b: &mut test::Bencher) {
    fn fact(n: u32) -> BigUint {
        (1..n + 1)
            .into_par_iter()
            .fold_chunks_with(64, BigUint::from(1_u32), |acc, x| acc.mul(x))
            .reduce_with(Mul::mul)
            .unwrap()
    }

    let f = factorial(N);
    b.iter(|| assert_eq!(fact(test::black_box(N)), f));
}

#[bench]
/// Compute the Factorial using divide-and-conquer serial recursion.
fn factorial_recursion(b: &mut test::Bencher) {
    fn product(a: u32, b: u32) -> BigUint {
        if a == b {
            return a.into();
        }
        let mid = (a + b) / 2;
        product(a, mid) * product(mid + 1, b)
    }

    let f = factorial(N);
    b.iter(|| assert_eq!(product(1, test::black_box(N)), f));
}

#[bench]
/// Compute the Factorial using divide-and-conquer parallel join.
fn factorial_join(b: &mut test::Bencher) {
    fn product(a: u32, b: u32) -> BigUint {
        if a == b {
            return a.into();
        }
        let mid = (a + b) / 2;
        let (x, y) = rayon::join(|| product(a, mid), || product(mid + 1, b));
        x * y
    }

    let f = factorial(N);
    b.iter(|| assert_eq!(product(1, test::black_box(N)), f));
}
