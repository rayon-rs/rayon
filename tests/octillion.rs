extern crate rayon;

use rayon::prelude::*;

const OCTILLION: u128 = 1_000_000_000_000_000_000_000_000_000;

/// Produce a parallel iterator for 0u128..10²⁷
fn octillion() -> rayon::range::Iter<u128> {
    (0..OCTILLION).into_par_iter()
}

/// Produce a parallel iterator for 0u128..10²⁷ using `flat_map`
fn octillion_flat() -> impl ParallelIterator<Item = u128> {
    (0u32..1_000_000_000)
        .into_par_iter()
        .with_max_len(1_000)
        .map(|i| i as u64 * 1_000_000_000)
        .flat_map(
            |i| {
                (0u32..1_000_000_000)
                    .into_par_iter()
                    .with_max_len(1_000)
                    .map(move |j| i + j as u64)
            }
        )
        .map(|i| i as u128 * 1_000_000_000)
        .flat_map(
            |i| {
                (0u32..1_000_000_000)
                    .into_par_iter()
                    .with_max_len(1_000)
                    .map(move |j| i + j as u128)
            }
        )
}

#[test]
fn find_first_octillion() {
    let x = octillion().find_first(|_| true);
    assert_eq!(x, Some(0));
}

#[test]
fn find_first_octillion_flat() {
    let x = octillion_flat().find_first(|_| true);
    assert_eq!(x, Some(0));
}

#[test]
fn find_last_octillion() {
    // FIXME: If we don't use at least two threads, then we end up walking
    // through the entire iterator sequentially, without the benefit of any
    // short-circuiting.  We probably don't want testing to wait that long. ;)
    // It would be nice if `find_last` could prioritize the later splits,
    // basically flipping the `join` args, without needing indexed `rev`.
    // (or could we have an unindexed `rev`?)
    let builder = rayon::ThreadPoolBuilder::new().num_threads(2);
    let pool = builder.build().unwrap();

    let x = pool.install(|| octillion().find_last(|_| true));
    assert_eq!(x, Some(OCTILLION - 1));
}

#[test]
fn find_last_octillion_flat() {
    // FIXME: Ditto, need two threads.
    let builder = rayon::ThreadPoolBuilder::new().num_threads(2);
    let pool = builder.build().unwrap();

    let x = pool.install(|| octillion_flat().find_last(|_| true));
    assert_eq!(x, Some(OCTILLION - 1));
}
