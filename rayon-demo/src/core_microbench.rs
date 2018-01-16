//! Some microbenchmarks that stress test a pure `join` path.

use rayon;
use rayon_core::registry::{self, Registry};
use rayon_core::latch::Latch;
use rayon_core::fiber::WaiterLatch;
use rayon::prelude::*;
use test::Bencher;
use std::usize;

#[bench]
fn unused_latch(b: &mut Bencher) {
    b.iter(|| {
        for i in 0..700 {
            let mut latch = WaiterLatch::new();
            latch.set();
            Registry::current().signal();
        }
    });
}

#[bench]
fn join_overhead(b: &mut Bencher) {
    b.iter(|| {
        registry::in_worker(|_, _| {
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
            registry::in_worker(|_, _| {
                unsafe { asm!(""::::"volatile") };
            });
        }
    });
}
