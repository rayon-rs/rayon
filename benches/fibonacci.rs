//! Benchmark Fibonacci numbers, F(n) = F(n-1) + F(n-2)
//!
//! Recursion is a horrible way to compute this -- roughly O(2ⁿ).
//!
//! It's potentially interesting for rayon::join, because the splits are
//! unequal.  F(n-1) has roughly twice as much work to do as F(n-2).  The
//! imbalance might make it more likely to leave idle threads ready to steal
//! jobs.  We can also see if there's any effect to having the larger job first
//! or second.
//!
//! We're doing very little real work in each job, so the rayon overhead is
//! going to dominate.  The serial recursive version will likely be faster,
//! unless you have a whole lot of CPUs.  The iterative version reveals the
//! joke.

#![feature(test)]

extern crate rayon;
extern crate test;

use rayon::Configuration;

const INIT_FAILED: &'static str = "Rayon failed to initialize";
const N: u32 = 32;
const FN: u32 = 2178309;


#[bench]
/// Compute the Fibonacci number recursively, without any parallelism.
fn fibonacci_recursive(b: &mut test::Bencher) {

    fn fib(n: u32) -> u32 {
        if n < 2 { return n; }

        fib(n - 1) + fib(n - 2)
    }

    b.iter(|| assert_eq!(fib(test::black_box(N)), FN));
}


#[bench]
/// Compute the Fibonacci number recursively, using rayon::join.
/// The larger branch F(N-1) is computed first.
fn fibonacci_join_1_2(b: &mut test::Bencher) {
    rayon::initialize(Configuration::new())
                     .expect(INIT_FAILED);

    fn fib(n: u32) -> u32 {
        if n < 2 { return n; }

        let (a, b) = rayon::join(
            || fib(n - 1),
            || fib(n - 2));
        a + b
    }

    b.iter(|| assert_eq!(fib(test::black_box(N)), FN));
}


#[bench]
/// Compute the Fibonacci number recursively, using rayon::join.
/// The smaller branch F(N-2) is computed first.
fn fibonacci_join_2_1(b: &mut test::Bencher) {
    rayon::initialize(Configuration::new())
                     .expect(INIT_FAILED);

    fn fib(n: u32) -> u32 {
        if n < 2 { return n; }

        let (a, b) = rayon::join(
            || fib(n - 2),
            || fib(n - 1));
        a + b
    }

    b.iter(|| assert_eq!(fib(test::black_box(N)), FN));
}


#[bench]
/// Compute the Fibonacci number iteratively, just to show how silly the others
/// are.  Parallelism can't make up for a bad choice of algorithm.
fn fibonacci_iterative(b: &mut test::Bencher) {

    fn fib(n: u32) -> u32 {
        let mut a = 0;
        let mut b = 1;
        for _ in 0..n {
            let c = a + b;
            a = b;
            b = c;
        }
        a
    }

    b.iter(|| assert_eq!(fib(test::black_box(N)), FN));
}
