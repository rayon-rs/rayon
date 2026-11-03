//! Memory PoCs. These PROVE the buffering claims with a tracking global
//! allocator instead of asserting them from the code shape. All claims run in a
//! SINGLE test (sequential) after a pool warm-up, so lazy pool init and the
//! default concurrent test runner can't contaminate the shared counters.
//!
//!   CLAIM 2 : the drive path buffers only the inner input. Peak auxiliary
//!             memory must NOT scale with the outer length.
//!   CLAIM 7b: an opaque (opt_len == None) outer is ALSO streamed, not buffered.
//!   CLAIM 6 : collect_into_vec (the indexed producer path) DOES buffer both, so
//!             it uses ~O(outer) more auxiliary memory than plain collect.
use rayon::prelude::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Tracking;
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let cur = CURRENT.fetch_add(layout.size(), Relaxed) + layout.size();
            PEAK.fetch_max(cur, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            let old = layout.size();
            if new_size >= old {
                let cur = CURRENT.fetch_add(new_size - old, Relaxed) + (new_size - old);
                PEAK.fetch_max(cur, Relaxed);
            } else {
                CURRENT.fetch_sub(old - new_size, Relaxed);
            }
        }
        p
    }
}

#[global_allocator]
static ALLOC: Tracking = Tracking;

/// Peak *additional* bytes allocated above the level at entry, over the call.
fn measure<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let start = CURRENT.load(Relaxed);
    PEAK.store(start, Relaxed);
    let out = f();
    let peak = PEAK.load(Relaxed).saturating_sub(start);
    (out, peak)
}

fn expected_sum(outer: u64, inner: u64) -> u64 {
    let sum_a = outer * outer.saturating_sub(1) / 2;
    let sum_b = inner * inner.saturating_sub(1) / 2;
    inner * sum_a + outer * sum_b
}

#[test]
fn memory_claims() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    // Warm up so lazy per-thread/pool allocation is not attributed to a measurement.
    pool.install(|| (0..2_000_000u64).into_par_iter().sum::<u64>());

    // ---- CLAIM 2: inner-only. Grow the outer 8x; auxiliary must stay ~flat.
    // A buffered outer would grow by ~8 bytes * 3.5M ~= 28 MB.
    let inner = 4u64;
    let (s_small, peak_small) = measure(|| {
        pool.install(|| {
            (0..500_000u64)
                .into_par_iter()
                .cartesian_product(0..inner)
                .map(|(a, b)| a + b)
                .sum::<u64>()
        })
    });
    let (s_large, peak_large) = measure(|| {
        pool.install(|| {
            (0..4_000_000u64)
                .into_par_iter()
                .cartesian_product(0..inner)
                .map(|(a, b)| a + b)
                .sum::<u64>()
        })
    });
    assert_eq!(s_small, expected_sum(500_000, inner));
    assert_eq!(s_large, expected_sum(4_000_000, inner));
    println!("CLAIM2 inner-only sum: peak_small={peak_small} peak_large={peak_large}");
    assert!(
        peak_small < 1 << 20,
        "claim2 small peak {peak_small} too high"
    );
    assert!(
        peak_large < 1 << 20,
        "claim2 large peak {peak_large} too high (outer buffered?)"
    );

    // ---- CLAIM 7b: an opaque outer (filter -> opt_len None) is also streamed.
    let (c_small, peak_op_small) = measure(|| {
        pool.install(|| {
            (0..500_000u64)
                .into_par_iter()
                .filter(|_| true)
                .cartesian_product(0..inner)
                .count()
        })
    });
    let (c_large, peak_op_large) = measure(|| {
        pool.install(|| {
            (0..4_000_000u64)
                .into_par_iter()
                .filter(|_| true)
                .cartesian_product(0..inner)
                .count()
        })
    });
    assert_eq!(c_small, 500_000 * inner as usize);
    assert_eq!(c_large, 4_000_000 * inner as usize);
    println!("CLAIM7b opaque outer count: peak_small={peak_op_small} peak_large={peak_op_large}");
    assert!(
        peak_op_large < 1 << 20,
        "claim7b large peak {peak_op_large} too high (outer buffered?)"
    );

    // ---- CLAIM 6: collect_into_vec buffers BOTH. With inner==1 (equal output
    // size), it uses ~O(outer) more auxiliary memory than plain collect.
    let outer = 1_000_000usize;
    let (v1, peak_collect) = measure(|| {
        pool.install(|| {
            (0..outer)
                .into_par_iter()
                .cartesian_product(0..1usize)
                .collect::<Vec<(usize, usize)>>()
        })
    });
    let (v2, peak_into_vec) = measure(|| {
        pool.install(|| {
            let mut v = Vec::new();
            (0..outer)
                .into_par_iter()
                .cartesian_product(0..1usize)
                .collect_into_vec(&mut v);
            v
        })
    });
    assert_eq!(v1.len(), outer);
    assert_eq!(v2.len(), outer);
    let extra = peak_into_vec.saturating_sub(peak_collect);
    println!("CLAIM6 collect={peak_collect} into_vec={peak_into_vec} extra={extra}");
    // collect is inner-only: its peak is ~the output only (no O(outer) input buffer).
    assert!(
        peak_collect < (outer * 20),
        "collect peak {peak_collect} suggests it buffered the outer too"
    );
    // collect_into_vec buffers the outer input: ~8 bytes/elem extra.
    assert!(
        extra > (outer * 4),
        "collect_into_vec did not use the expected ~O(outer) extra buffer (extra={extra})"
    );
}
