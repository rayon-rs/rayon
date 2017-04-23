use test;

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
fn merge_sort_par_bench(b: &mut test::Bencher) {
    bench_harness(super::merge_sort, b);
}

#[bench]
fn merge_sort_seq_bench(b: &mut test::Bencher) {
    bench_harness(super::seq_merge_sort, b);
}
