//! How many Pythagorean triples exist less than or equal to a million?
//! i.e. a²+b²=c² and a,b,c ≤ 1000000

#![feature(test)]

extern crate num;
extern crate rayon;
extern crate test;

use num::Integer;
use rayon::prelude::*;
use rayon::Configuration;
use std::ops::Add;
use std::usize::MAX;

const INIT_FAILED: &'static str = "Rayon failed to initialize";

/// Use Euclid's formula to count Pythagorean triples
///
/// https://en.wikipedia.org/wiki/Pythagorean_triple#Generating_a_triple
///
/// For coprime integers m and n, with m > n and m-n is odd, then
///     a = m²-n², b = 2mn, c = m²+n²
///
/// This is a coprime triple.  Multiplying by factors k covers all triples.
fn par_euclid(m_threshold: usize, n_threshold: usize) -> u32 {
    (1u32 .. 1000).into_par_iter().sequential_threshold(m_threshold).map(|m| {
        (1 .. m).into_par_iter().sequential_threshold(n_threshold)
            .filter(|n| (m - n).is_odd() && m.gcd(n) == 1)
            .map(|n| 1000000 / (m*m + n*n))
            .sum()
    }).sum()
}

/// Same as par_euclid, without using rayon.
fn euclid() -> u32 {
    (1u32 .. 1000).map(|m| {
        (1 .. m)
            .filter(|n| (m - n).is_odd() && m.gcd(n) == 1)
            .map(|n| 1000000 / (m*m + n*n))
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
/// Use max thresholds to force it fully serialized.
fn euclid_faux_serial(b: &mut test::Bencher) {
    rayon::initialize(Configuration::new())
                     .expect(INIT_FAILED);

    let count = euclid();
    b.iter(|| assert_eq!(par_euclid(MAX, MAX), count))
}

#[bench]
/// Use infinite weight to force the outer loop parallelized,
/// but use MAX to make inner loop serial.
fn euclid_parallel_outer(b: &mut test::Bencher) {
    rayon::initialize(Configuration::new())
                     .expect(INIT_FAILED);

    let count = euclid();
    b.iter(|| assert_eq!(par_euclid(1, MAX), count))
}

#[bench]
/// Use 1 cutoff for both loops to force it fully parallelized.
fn euclid_parallel_full(b: &mut test::Bencher) {
    rayon::initialize(Configuration::new())
                     .expect(INIT_FAILED);

    let count = euclid();
    b.iter(|| assert_eq!(par_euclid(1, 1), count))
}
