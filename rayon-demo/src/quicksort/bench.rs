use test;

use super::{Parallel, Sequential};

// Size to use when doing `cargo bench`; extensively tuned to run in
// "not too long" on my laptop -nmatsakis
const BENCH_SIZE: usize = 250_000_000 / 512;

fn bench_harness<F: FnMut(&mut [u32])>(mut f: F, b: &mut test::Bencher) {
    let base_vec = super::default_vec(BENCH_SIZE);
    let mut sort_vec = vec![];
    b.iter(
        || {
            sort_vec = base_vec.clone();
            f(&mut sort_vec);
        },
    );
    assert!(super::is_sorted(&mut sort_vec));
}

#[bench]
fn quick_sort_par_bench(b: &mut test::Bencher) {
    bench_harness(super::quick_sort::<Parallel, u32>, b);
}

#[bench]
fn quick_sort_seq_bench(b: &mut test::Bencher) {
    bench_harness(super::quick_sort::<Sequential, u32>, b);
}

#[bench]
fn quick_sort_splitter(b: &mut test::Bencher) {
    use rayon::iter::ParallelIterator;

    bench_harness(
        |vec| {
            ::rayon::split(
                vec, |vec| if vec.len() > 1 {
                    let mid = super::partition(vec);
                    let (left, right) = vec.split_at_mut(mid);
                    (left, Some(right))
                } else {
                    (vec, None)
                }
            )
                    .for_each(super::quick_sort::<Sequential, u32>)
        },
        b,
    );
}
