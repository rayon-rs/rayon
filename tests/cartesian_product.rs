//! Behaviour tests for `ParallelIterator::cartesian_product`. Each block notes
//! the property it pins down. Memory behaviour (inner-only buffering) is proven
//! separately in `cartesian_memory.rs`, which needs a global allocator.
use rayon::ThreadPoolBuilder;
use rayon::iter::plumbing::UnindexedConsumer;
use rayon::prelude::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The reference (sequential) cartesian product, row-major.
fn seq(m: usize, n: usize) -> Vec<(usize, usize)> {
    (0..m).flat_map(|a| (0..n).map(move |b| (a, b))).collect()
}

// ---- Row-major order, all small size pairs, on both indexed and unindexed
// inputs. `u64` ranges are NOT `IndexedParallelIterator`; the method now lives on
// `ParallelIterator`, so they work (the previous version could not compile them).
#[test]
fn order_matches_sequential_indexed_and_unindexed() {
    for m in 0..7 {
        for n in 0..7 {
            let indexed: Vec<(usize, usize)> =
                (0..m).into_par_iter().cartesian_product(0..n).collect();
            assert_eq!(indexed, seq(m, n), "usize {m}x{n}");

            let unindexed: Vec<(u64, u64)> = (0..m as u64)
                .into_par_iter()
                .cartesian_product(0..n as u64)
                .collect();
            let want: Vec<(u64, u64)> = seq(m, n)
                .iter()
                .map(|&(a, b)| (a as u64, b as u64))
                .collect();
            assert_eq!(unindexed, want, "u64 {m}x{n}");
        }
    }
}

// ---- Mixed indexed/unindexed inputs in either position.
#[test]
fn mixed_indexed_and_unindexed_inputs() {
    let a: Vec<(usize, u64)> = (0..3usize)
        .into_par_iter()
        .cartesian_product(0..2u64)
        .collect();
    assert_eq!(a, vec![(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1)]);

    let b: Vec<(u64, usize)> = (0..2u64)
        .into_par_iter()
        .cartesian_product(0..3usize)
        .collect();
    assert_eq!(b, vec![(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]);
}

// ---- opt_len is the product of the two lengths when both are known (this is
// what lets ordinary `collect` preallocate), and None when a length is unknown.
#[test]
fn opt_len_is_the_product_when_both_known() {
    assert_eq!(
        (0..3u64)
            .into_par_iter()
            .cartesian_product(0..4u64)
            .opt_len(),
        Some(12)
    );
    assert_eq!(
        (0..3usize)
            .into_par_iter()
            .cartesian_product(0..4usize)
            .opt_len(),
        Some(12)
    );
    // exact-but-unindexed outer: a `map` over a range forwards opt_len.
    assert_eq!(
        (0..4u64)
            .into_par_iter()
            .map(|x| x * 10)
            .cartesian_product(0..2u64)
            .opt_len(),
        Some(8)
    );
    // opaque outer: `filter` has opt_len == None, so the product does too.
    assert_eq!(
        (0..5u64)
            .into_par_iter()
            .filter(|_| true)
            .cartesian_product(0..2u64)
            .opt_len(),
        None
    );
}

// ---- An exact-but-unindexed outer (map over a range) still collects correctly.
#[test]
fn exact_adaptor_outer_is_correct() {
    let got: Vec<(u64, u64)> = (0..4u64)
        .into_par_iter()
        .map(|x| x * 10)
        .cartesian_product(0..2u64)
        .collect();
    let want: Vec<(u64, u64)> = (0..4u64)
        .flat_map(|a| (0..2u64).map(move |b| (a * 10, b)))
        .collect();
    assert_eq!(got, want);
}

// ---- An opaque outer (filter -> opt_len None) still works and stays ordered.
// It just does not preallocate. It is NOT forced to buffer the outer (see
// `cartesian_memory.rs`).
#[test]
fn opaque_outer_is_correct() {
    let got: Vec<(u64, u64)> = (0..5u64)
        .into_par_iter()
        .filter(|&a| a % 2 == 0)
        .cartesian_product(0..2u64)
        .collect();
    let want: Vec<(u64, u64)> = [0u64, 2, 4]
        .iter()
        .flat_map(|&a| (0..2u64).map(move |b| (a, b)))
        .collect();
    assert_eq!(got, want);
}

/// Exact-length, deliberately NON-indexed wrapper: it honours the `opt_len`
/// contract by driving exact consumers only through `Consumer::split_at`. Used to
/// check the inner-only exact path never reaches `CollectConsumer::split_off_left`.
struct ExactUnindexed<I>(I);

impl<I> ParallelIterator for ExactUnindexed<I>
where
    I: IndexedParallelIterator,
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.0.drive(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.0.len())
    }
}

#[test]
fn exact_nonindexed_outer_collects_via_split_at() {
    let got: Vec<_> = ExactUnindexed((0..257).into_par_iter())
        .cartesian_product(0..7)
        .collect();
    assert_eq!(got, seq(257, 7));
}

// ---- When BOTH inputs are indexed, the product is itself indexed: the whole
// indexed API works and matches flat row-major indices, including mid-row splits.
#[test]
fn indexed_api_matches_flat_indices() {
    let expected = seq(73, 13);

    let mut into = Vec::new();
    (0..73)
        .into_par_iter()
        .cartesian_product(0..13)
        .collect_into_vec(&mut into);
    assert_eq!(into, expected);

    assert_eq!(
        (0..73).into_par_iter().cartesian_product(0..13).len(),
        73 * 13
    );

    let enumerated: Vec<_> = (0..73)
        .into_par_iter()
        .cartesian_product(0..13)
        .enumerate()
        .collect();
    assert_eq!(
        enumerated,
        expected.iter().copied().enumerate().collect::<Vec<_>>()
    );

    let reversed: Vec<_> = (0..73)
        .into_par_iter()
        .cartesian_product(0..13)
        .rev()
        .collect();
    assert_eq!(reversed, expected.iter().copied().rev().collect::<Vec<_>>());

    let sliced: Vec<_> = (0..73)
        .into_par_iter()
        .cartesian_product(0..13)
        .skip(17)
        .take(811)
        .collect();
    assert_eq!(sliced, expected[17..828]);

    let zipped: Vec<_> = (0..73)
        .into_par_iter()
        .cartesian_product(0..13)
        .zip(10_000..10_000 + expected.len())
        .collect();
    assert_eq!(
        zipped,
        expected
            .iter()
            .copied()
            .zip(10_000..10_000 + expected.len())
            .collect::<Vec<_>>()
    );

    let blocks: Vec<_> = (0..73)
        .into_par_iter()
        .cartesian_product(0..13)
        .by_exponential_blocks()
        .collect();
    assert_eq!(blocks, expected);
}

// ---- Early termination through the streamed outer / expanded inner.
#[test]
fn early_termination_is_correct() {
    assert!(
        (0..67)
            .into_par_iter()
            .cartesian_product(0..11)
            .any(|x| x == (32, 5))
    );
    assert!(
        (0..67)
            .into_par_iter()
            .cartesian_product(0..11)
            .all(|(a, b)| a < 67 && b < 11)
    );
    assert_eq!(
        (0..67)
            .into_par_iter()
            .cartesian_product(0..11)
            .find_first(|x| *x == (32, 5)),
        Some((32, 5))
    );
    assert_eq!(
        (0..67)
            .into_par_iter()
            .cartesian_product(0..11)
            .position_any(|x| x == (32, 5)),
        Some(32 * 11 + 5)
    );

    let taken: Vec<_> = (0..67)
        .into_par_iter()
        .cartesian_product(0..11)
        .take(32 * 11 + 6)
        .collect();
    assert_eq!(taken, seq(67, 11)[..32 * 11 + 6]);

    let any_taken: Vec<_> = (0..67)
        .into_par_iter()
        .cartesian_product(0..11)
        .take_any(13)
        .collect();
    assert_eq!(any_taken.len(), 13);
}

// ---- `take_any(0)` wants nothing, so neither input is driven.
#[test]
fn take_any_zero_drives_nothing() {
    let result = catch_unwind(|| {
        (0..4)
            .into_par_iter()
            .cartesian_product(
                (0..4)
                    .into_par_iter()
                    .map(|_| -> usize { panic!("take_any(0) evaluated the inner iterator") }),
            )
            .take_any(0)
            .collect::<Vec<_>>()
    });
    assert!(result.is_ok(), "take_any(0) should not drive its input");
    assert!(result.unwrap().is_empty());
}

// ---- Empty axes, the known-empty short-circuit, ZST items, non-Copy items.
#[test]
fn empty_zst_and_non_copy() {
    assert!(
        (0..0)
            .into_par_iter()
            .cartesian_product(0..1_000)
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(
        (0..1_000)
            .into_par_iter()
            .cartesian_product(0..0)
            .collect::<Vec<_>>()
            .is_empty()
    );

    let zst: Vec<_> = vec![(); 257]
        .into_par_iter()
        .cartesian_product(vec![(); 31])
        .collect();
    assert_eq!(zst.len(), 257 * 31);

    let rows = vec!["a".to_string(), "b".to_string()];
    let cols = vec!["x".to_string(), "y".to_string()];
    let got: Vec<(String, String)> = rows.par_iter().cloned().cartesian_product(cols).collect();
    assert_eq!(
        got,
        vec![
            ("a".into(), "x".into()),
            ("a".into(), "y".into()),
            ("b".into(), "x".into()),
            ("b".into(), "y".into()),
        ]
    );
}

// ---- Nesting, and no leaks/double-drops when a closure panics mid-product.
#[test]
fn nested_and_panic_safe() {
    let nested: Vec<Vec<_>> = (0..17)
        .into_par_iter()
        .map(|offset| {
            (0..19)
                .into_par_iter()
                .cartesian_product(0..5)
                .map(|(a, b)| (offset, a, b))
                .collect()
        })
        .collect();
    for (offset, row) in nested.into_iter().enumerate() {
        let expected: Vec<_> = (0..19)
            .flat_map(|a| (0..5).map(move |b| (offset, a, b)))
            .collect();
        assert_eq!(row, expected);
    }

    let live = Arc::new(AtomicUsize::new(0));
    struct Tracked(Arc<AtomicUsize>);
    impl Clone for Tracked {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::SeqCst);
            Self(Arc::clone(&self.0))
        }
    }
    impl Tracked {
        fn new(live: &Arc<AtomicUsize>) -> Self {
            live.fetch_add(1, Ordering::SeqCst);
            Self(Arc::clone(live))
        }
    }
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    let outer: Vec<_> = (0..64).map(|_| Tracked::new(&live)).collect();
    let inner: Vec<_> = (0..16).map(|_| Tracked::new(&live)).collect();
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        outer
            .into_par_iter()
            .cartesian_product(inner)
            .for_each(|_| panic!("intentional mid-product panic"));
    }));
    assert!(panicked.is_err());
    assert_eq!(live.load(Ordering::SeqCst), 0);
}

// ---- Row-major order holds for random size pairs across thread-pool sizes.
#[test]
fn order_is_row_major_across_thread_counts() {
    for threads in [1, 2, 3, 4, 7] {
        ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let mut seed = 0x9e37_79b9_u32;
                for _ in 0..100 {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let m = (seed as usize) % 97;
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let n = (seed as usize) % 41;
                    let got: Vec<_> = (0..m).into_par_iter().cartesian_product(0..n).collect();
                    assert_eq!(got, seq(m, n), "threads={threads} {m}x{n}");
                }
            });
    }
}

// ---- A 500x300 product summed in parallel matches the closed form.
#[test]
fn large_parallel_sum() {
    let (m, n) = (500usize, 300usize);
    let sum: u64 = (0..m)
        .into_par_iter()
        .cartesian_product(0..n)
        .map(|(a, b)| (a * b) as u64)
        .sum();
    let expected: u64 = (0..m)
        .flat_map(|a| (0..n).map(move |b| (a * b) as u64))
        .sum();
    assert_eq!(sum, expected);
}
