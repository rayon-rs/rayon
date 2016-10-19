//! How many Pythagorean triples exist less than or equal to a million?
//! i.e. a²+b²=c² and a,b,c ≤ 1000000

#![feature(test)]

use num::Integer;
use rayon::prelude::*;
use std::f64::INFINITY;
use std::ops::Add;
use test;

/// Use Euclid's formula to count Pythagorean triples
///
/// https://en.wikipedia.org/wiki/Pythagorean_triple#Generating_a_triple
///
/// For coprime integers m and n, with m > n and m-n is odd, then
///     a = m²-n², b = 2mn, c = m²+n²
///
/// This is a coprime triple.  Multiplying by factors k covers all triples.
fn par_euclid(m_weight: f64, n_weight: f64) -> u32 {
    (1u32 .. 2000).into_par_iter().weight(m_weight).map(|m| {
        (1 .. m).into_par_iter().weight(n_weight)
            .filter(|n| (m - n).is_odd() && m.gcd(n) == 1)
            .map(|n| 4000000 / (m*m + n*n))
            .sum()
    }).sum()
}

/// Same as par_euclid, without explicit weights.
fn par_euclid_weightless() -> u32 {
    (1u32 .. 2000).into_par_iter().map(|m| {
        (1 .. m).into_par_iter()
            .filter(|n| (m - n).is_odd() && m.gcd(n) == 1)
            .map(|n| 4000000 / (m*m + n*n))
            .sum()
    }).sum()
}

/// Same as par_euclid, without using rayon.
fn euclid() -> u32 {
    (1u32 .. 2000).map(|m| {
        (1 .. m)
            .filter(|n| (m - n).is_odd() && m.gcd(n) == 1)
            .map(|n| 4000000 / (m*m + n*n))
            .fold(0, Add::add)
    }).fold(0, Add::add)
}

#[bench]
/// Benchmark without rayon at all
fn euclid_serial(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(euclid(), count))
}

#[bench]
/// Use zero weights to force it fully serialized.
fn euclid_faux_serial(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(par_euclid(0.0, 0.0), count))
}

#[bench]
/// Use the default without any weights
fn euclid_parallel_weightless(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(par_euclid_weightless(), count))
}

#[bench]
/// Use the default weights (1.0)
fn euclid_parallel_one(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(par_euclid(1.0, 1.0), count))
}

#[bench]
/// Use infinite weight to force the outer loop parallelized.
fn euclid_parallel_outer(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(par_euclid(INFINITY, 1.0), count))
}

#[bench]
/// Use infinite weights to force it fully parallelized.
fn euclid_parallel_full(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(par_euclid(INFINITY, INFINITY), count))
}
