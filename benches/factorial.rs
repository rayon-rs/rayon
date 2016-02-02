//! Benchmark Factorial N! = 1×2×⋯×N

#![feature(test)]

extern crate num;
extern crate rayon;
extern crate test;

use num::{One, BigUint};
use rayon::par_iter::*;
use rayon::Configuration;
use std::ops::Mul;

const INIT_FAILED: &'static str = "Rayon failed to initialize";
const N: u32 = 9999;

/// Compute the Factorial using a plain iterator.
fn factorial(n: u32) -> BigUint {
    (1 .. n + 1)
        .map(BigUint::from)
        .fold(BigUint::one(), Mul::mul)
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
    rayon::initialize(Configuration::new())
                     .expect(INIT_FAILED);

    fn fact(n: u32) -> BigUint {
        (1 .. n + 1).into_par_iter().weight_max()
            .map(BigUint::from)
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
        if a == b { return a.into(); }
        let mid = (a + b) / 2;
        product(a, mid) * product(mid + 1, b)
    }

    let f = factorial(N);
    b.iter(|| assert_eq!(product(1, test::black_box(N)), f));
}


#[bench]
/// Compute the Factorial using divide-and-conquer parallel join.
fn factorial_join(b: &mut test::Bencher) {
    rayon::initialize(Configuration::new())
                     .expect(INIT_FAILED);

    fn product(a: u32, b: u32) -> BigUint {
        if a == b { return a.into(); }
        let mid = (a + b) / 2;
        let (x, y) = rayon::join(
            || product(a, mid),
            || product(mid + 1, b));
        x * y
    }

    let f = factorial(N);
    b.iter(|| assert_eq!(product(1, test::black_box(N)), f));
}
