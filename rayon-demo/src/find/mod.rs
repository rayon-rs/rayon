/// Simple benchmarks of `find()` performance

use rayon::prelude::*;
use rand::{Rng, SeedableRng, XorShiftRng};
use test::Bencher;


lazy_static! {
    static ref HAYSTACK: Vec<u32> = {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        (0..10_000_000).map(|_| rng.next_u32()).collect()
    };
}


#[bench]
fn parallel_find_first(b: &mut Bencher) {
    let needle = HAYSTACK[0];
    b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x == needle).is_some()));
}

#[bench]
fn serial_find_first(b: &mut Bencher) {
    let needle = HAYSTACK[0];
    b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x == needle).is_some()));
}


#[bench]
fn parallel_find_last(b: &mut Bencher) {
    let needle = HAYSTACK[HAYSTACK.len()-1];
    b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x == needle).is_some()));
}

#[bench]
fn serial_find_last(b: &mut Bencher) {
    let needle = HAYSTACK[HAYSTACK.len()-1];
    b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x == needle).is_some()));
}


#[bench]
fn parallel_find_middle(b: &mut Bencher) {
    let needle = HAYSTACK[HAYSTACK.len() / 3 * 2];
    b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x == needle).is_some()));
}

#[bench]
fn serial_find_middle(b: &mut Bencher) {
    let needle = HAYSTACK[HAYSTACK.len() / 3 * 2];
    b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x == needle).is_some()));
}


#[bench]
fn parallel_find_missing(b: &mut Bencher) {
    let needle = HAYSTACK.iter().max().unwrap() + 1;
    b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x == needle).is_none()));
}

#[bench]
fn serial_find_missing(b: &mut Bencher) {
    let needle = HAYSTACK.iter().max().unwrap() + 1;
    b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x == needle).is_none()));
}


#[bench]
fn parallel_find_common(b: &mut Bencher) {
    b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x % 1000 == 999).is_some()));
}

#[bench]
fn serial_find_common(b: &mut Bencher) {
    b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x % 1000 == 999).is_some()));
}
