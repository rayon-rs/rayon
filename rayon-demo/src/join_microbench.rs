//! Some microbenchmarks that stress test a pure `join` path.

use rayon;
use rayon::prelude::*;
use test::Bencher;
use std::usize;

#[bench]
fn increment_all(b: &mut Bencher) {
    let mut big_vec = vec![0_usize; 100*1024];
    b.iter(
        || {
            big_vec
                .par_iter_mut()
                .for_each(|p| *p = p.wrapping_add(1));
        },
    );
}

#[bench]
fn increment_all_min(b: &mut Bencher) {
    let mut big_vec = vec![0_usize; 100*1024];
    b.iter(
        || {
            big_vec
                .par_iter_mut()
                .with_min_len(1024)
                .for_each(|p| *p = p.wrapping_add(1));
        },
    );
}

#[bench]
fn increment_all_serialized(b: &mut Bencher) {
    let mut big_vec = vec![0_usize; 100*1024];
    b.iter(
        || {
            big_vec
                .par_iter_mut()
                .with_min_len(usize::MAX)
                .for_each(|p| *p = p.wrapping_add(1));
        },
    );
}

#[bench]
fn increment_all_max(b: &mut Bencher) {
    let mut big_vec = vec![0_usize; 100*1024];
    b.iter(
        || {
            big_vec
                .par_iter_mut()
                .with_max_len(100)
                .for_each(|p| *p = p.wrapping_add(1));
        },
    );
}

#[bench]
fn increment_all_atomized(b: &mut Bencher) {
    let mut big_vec = vec![0_usize; 100*1024];
    b.iter(
        || {
            big_vec
                .par_iter_mut()
                .with_max_len(1)
                .for_each(|p| *p = p.wrapping_add(1));
        },
    );
}

#[bench]
fn join_recursively(b: &mut Bencher) {
    fn join_recursively(n: usize) {
        if n == 0 {
            return;
        }
        rayon::join(|| join_recursively(n - 1), || join_recursively(n - 1));
    }

    b.iter(|| { join_recursively(16); });
}
