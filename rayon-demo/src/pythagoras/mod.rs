//! How many Pythagorean triples exist less than or equal to a million?
//! i.e. a²+b²=c² and a,b,c ≤ 1000000

use num::Integer;
use rayon::prelude::*;
use rayon::range::Iter;
use std::usize;
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
fn par_euclid<FM, M, FN, N>(map_m: FM, map_n: FN) -> u32
where
    FM: FnOnce(Iter<u32>) -> M,
    M: ParallelIterator<Item = u32>,
    FN: Fn(Iter<u32>) -> N + Sync,
    N: ParallelIterator<Item = u32>,
{
    map_m((1u32..2000).into_par_iter())
        .map(
            |m| -> u32 {
                map_n((1..m).into_par_iter())
                    .filter(|n| (m - n).is_odd() && m.gcd(n) == 1)
                    .map(|n| 4000000 / (m * m + n * n))
                    .sum()
            },
        )
        .sum()
}

/// Same as par_euclid, without tweaking split lengths
fn par_euclid_weightless() -> u32 {
    (1u32..2000)
        .into_par_iter()
        .map(
            |m| -> u32 {
                (1..m)
                    .into_par_iter()
                    .filter(|n| (m - n).is_odd() && m.gcd(n) == 1)
                    .map(|n| 4000000 / (m * m + n * n))
                    .sum()
            },
        )
        .sum()
}

/// Same as par_euclid, without using rayon.
fn euclid() -> u32 {
    (1u32..2000)
        .map(
            |m| {
                (1..m)
                    .filter(|n| (m - n).is_odd() && m.gcd(n) == 1)
                    .map(|n| 4000000 / (m * m + n * n))
                    .fold(0, Add::add)
            },
        )
        .fold(0, Add::add)
}

#[bench]
/// Benchmark without rayon at all
fn euclid_serial(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(euclid(), count))
}

#[bench]
/// Use huge minimums to force it fully serialized.
fn euclid_faux_serial(b: &mut test::Bencher) {
    let count = euclid();
    let serial = |r: Iter<u32>| r.with_min_len(usize::MAX);
    b.iter(|| assert_eq!(par_euclid(&serial, &serial), count))
}

#[bench]
/// Use the default without any weights
fn euclid_parallel_weightless(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(par_euclid_weightless(), count))
}

#[bench]
/// Use the default settings.
fn euclid_parallel_one(b: &mut test::Bencher) {
    let count = euclid();
    b.iter(|| assert_eq!(par_euclid(|m| m, |n| n), count))
}

#[bench]
/// Use a low maximum to force the outer loop parallelized.
fn euclid_parallel_outer(b: &mut test::Bencher) {
    let count = euclid();
    let parallel = |r: Iter<u32>| r.with_max_len(1);
    b.iter(|| assert_eq!(par_euclid(&parallel, |n| n), count))
}

#[bench]
/// Use low maximums to force it fully parallelized.
fn euclid_parallel_full(b: &mut test::Bencher) {
    let count = euclid();
    let parallel = |r: Iter<u32>| r.with_max_len(1);
    b.iter(|| assert_eq!(par_euclid(&parallel, &parallel), count))
}
