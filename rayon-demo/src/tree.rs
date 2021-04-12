//! Some benches for tree walks.
use rayon::prelude::*;

const SIZE: u64 = 100_000;
const VAL: u64 = SIZE * (SIZE - 1) / 2;

#[bench]
fn tree_prefix_collect(b: &mut ::test::Bencher) {
    let mut vec = None;
    b.iter(|| {
        vec = Some(
            rayon::iter::walk_tree_prefix(0u64..SIZE, |r| {
                // root is smallest
                let mid = (r.start + 1 + r.end) / 2;
                // large indices to the left, small to the right
                std::iter::once(mid..r.end)
                    .chain(std::iter::once((r.start + 1)..mid))
                    .filter(|r| !r.is_empty())
            })
            .map(|r| r.start)
            .collect::<Vec<_>>(),
        )
    });
    assert!(vec.unwrap().into_iter().eq(0..SIZE));
}

#[bench]
fn tree_postfix_collect(b: &mut ::test::Bencher) {
    let mut vec = None;
    b.iter(|| {
        vec = Some(
            rayon::iter::walk_tree_postfix(0u64..SIZE, |r| {
                // root is largest
                let mid = (r.start + r.end - 1) / 2;
                // small indices to the left, large to the right
                std::iter::once(r.start..mid)
                    .chain(std::iter::once(mid..(r.end - 1)))
                    .filter(|r| !r.is_empty())
            })
            .map(|r| r.end - 1)
            .collect::<Vec<_>>(),
        )
    });
    assert!(vec.unwrap().into_iter().eq(0..SIZE));
}

#[bench]
fn tree_prefix_sum(b: &mut ::test::Bencher) {
    b.iter(|| {
        let s = rayon::iter::walk_tree_prefix(0u64..SIZE, |r| {
            // root is smallest
            let mid = (r.start + 1 + r.end) / 2;
            // small indices to the left, large to the right
            std::iter::once((r.start + 1)..mid)
                .chain(std::iter::once(mid..r.end))
                .filter(|r| !r.is_empty())
        })
        .map(|r| r.start)
        .sum::<u64>();
        assert_eq!(s, VAL)
    })
}

#[bench]
fn tree_postfix_sum(b: &mut ::test::Bencher) {
    b.iter(|| {
        let s = rayon::iter::walk_tree_postfix(0u64..SIZE, |r| {
            // root is smallest
            let mid = (r.start + 1 + r.end) / 2;
            // small indices to the left, large to the right
            std::iter::once((r.start + 1)..mid)
                .chain(std::iter::once(mid..r.end))
                .filter(|r| !r.is_empty())
        })
        .map(|r| r.start)
        .sum::<u64>();
        assert_eq!(s, VAL)
    })
}
