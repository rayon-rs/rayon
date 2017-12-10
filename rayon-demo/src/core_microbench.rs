//! Some microbenchmarks that stress test a pure `join` path.

use rayon;
use rayon_core;
use rayon::prelude::*;
use test::Bencher;
use std::usize;

#[bench]
fn join_overhead(b: &mut Bencher) {
    b.iter(|| {
        rayon_core::registry::in_worker(|_, _| {
            for i in 0..70000 {
                rayon::join(|| {
                    unsafe { asm!(""::::"volatile") };
                }, || {
                    unsafe { asm!(""::::"volatile") };
                });
            }
        });
    });
}

#[bench]
fn install_overhead(b: &mut Bencher) {
    b.iter(|| {
        for i in 0..7000 {
            rayon_core::registry::in_worker(|_, _| {
                unsafe { asm!(""::::"volatile") };
            });
        }
    });
}
