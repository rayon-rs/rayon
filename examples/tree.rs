use rayon::prelude::*;

const SIZE: u64 = 10_000_000;
const VAL: u64 = SIZE * (SIZE - 1) / 2;

fn prefix_collect() {
    let start = std::time::Instant::now();
    let v: Vec<_> = rayon::iter::walk_tree_prefix(0u64..SIZE, |r| {
        // root is smallest
        let mid = (r.start + 1 + r.end) / 2;
        // large indices to the left, small to the right
        std::iter::once(mid..r.end)
            .chain(std::iter::once((r.start + 1)..mid))
            .filter(|r| !r.is_empty())
    })
    .map(|r| r.start)
    .collect();
    println!("prefix collect took {:?}", start.elapsed());
    assert!(v.into_iter().eq(0..SIZE));
}

fn postfix_collect() {
    let start = std::time::Instant::now();
    let v: Vec<_> = rayon::iter::walk_tree_postfix(0u64..SIZE, |r| {
        // root is largest
        let mid = (r.start + r.end - 1) / 2;
        // small indices to the left, large to the right
        std::iter::once(r.start..mid)
            .chain(std::iter::once(mid..(r.end - 1)))
            .filter(|r| !r.is_empty())
    })
    .map(|r| r.end - 1)
    .collect();
    println!("postfix collect took {:?}", start.elapsed());
    assert!(v.into_iter().eq(0..SIZE));
}

fn prefix_sum() {
    let start = std::time::Instant::now();
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
    println!("prefix sum took {:?}", start.elapsed());
    assert_eq!(s, VAL)
}
fn postfix_sum() {
    let start = std::time::Instant::now();
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
    println!("postfix sum took {:?}", start.elapsed());
    assert_eq!(s, VAL)
}

fn main() {
    prefix_collect();
    postfix_collect();
    prefix_sum();
    postfix_sum();
}
