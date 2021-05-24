/// Simple benchmarks of `find_any()` and `find_first` performance

macro_rules! make_tests {
    ($n:expr, $m:ident) => {
        mod $m {
            use rand::distributions::Standard;
            use rand::Rng;
            use rayon::prelude::*;
            use test::Bencher;

            lazy_static::lazy_static! {
                static ref HAYSTACK: Vec<[u32; $n]> = {
                    let rng = crate::seeded_rng();
                    rng.sample_iter(&Standard)
                        .map(|x| {
                            let mut result: [u32; $n] = [0; $n];
                            result[0] = x;
                            result
                        })
                        .take(10_000_000)
                        .collect()
                };
            }

            // this is a very dumb find_first algorithm.
            // no early aborts so we have a linear best case cost.
            fn find_dumb<I: ParallelIterator, P: Fn(&I::Item) -> bool + Send + Sync>(
                iter: I,
                cond: P,
            ) -> Option<I::Item> {
                iter.map(|e| if cond(&e) { Some(e) } else { None })
                    .reduce(|| None, |left, right| left.or(right))
            }

            #[bench]
            fn parallel_find_any_start(b: &mut Bencher) {
                let needle = HAYSTACK[0][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_first_start(b: &mut Bencher) {
                let needle = HAYSTACK[0][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_first_blocks_start(b: &mut Bencher) {
                let needle = HAYSTACK[0][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .by_exponential_blocks()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }

            #[bench]
            fn serial_find_start(b: &mut Bencher) {
                let needle = HAYSTACK[0][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_any_end(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() - 1][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }
            #[bench]
            fn parallel_find_first_end(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() - 1][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }
            #[bench]
            fn parallel_find_first_blocks_end(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() - 1][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .by_exponential_blocks()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }

            #[bench]
            fn serial_find_end(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() - 1][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_any_third(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_first_third(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }

            #[bench]
            fn parallel_find_dumb_third(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3][0];
                b.iter(
                    || assert!(find_dumb(HAYSTACK.par_iter(), (|&&x| x[0] == needle)).is_some()),
                );
            }

            #[bench]
            fn parallel_find_first_blocks_third(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .by_exponential_blocks()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }

            #[bench]
            fn serial_find_third(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_any_middle(b: &mut Bencher) {
                let needle = HAYSTACK[(HAYSTACK.len() / 2).saturating_sub(1)][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_first_middle(b: &mut Bencher) {
                let needle = HAYSTACK[(HAYSTACK.len() / 2).saturating_sub(1)][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }

            #[bench]
            fn parallel_find_dumb_middle(b: &mut Bencher) {
                let needle = HAYSTACK[(HAYSTACK.len() / 2).saturating_sub(1)][0];
                b.iter(
                    || assert!(find_dumb(HAYSTACK.par_iter(), (|&&x| x[0] == needle)).is_some()),
                );
            }

            #[bench]
            fn parallel_find_first_blocks_middle(b: &mut Bencher) {
                let needle = HAYSTACK[(HAYSTACK.len() / 2).saturating_sub(1)][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .by_exponential_blocks()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }

            #[bench]
            fn serial_find_middle(b: &mut Bencher) {
                let needle = HAYSTACK[(HAYSTACK.len() / 2).saturating_sub(1)][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_any_two_thirds(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3 * 2][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_first_two_thirds(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3 * 2][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }

            #[bench]
            fn parallel_find_first_blocks_two_thirds(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3 * 2][0];
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .by_exponential_blocks()
                        .find_first(|&&x| x[0] == needle)
                        .is_some())
                });
            }

            #[bench]
            fn serial_find_two_thirds(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3 * 2][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_any_missing(b: &mut Bencher) {
                let needle = HAYSTACK.iter().map(|v| v[0]).max().unwrap() + 1;
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_none()));
            }

            #[bench]
            fn parallel_find_first_missing(b: &mut Bencher) {
                let needle = HAYSTACK.iter().map(|v| v[0]).max().unwrap() + 1;
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .find_first(|&&x| x[0] == needle)
                        .is_none())
                });
            }

            #[bench]
            fn parallel_find_first_blocks_missing(b: &mut Bencher) {
                let needle = HAYSTACK.iter().map(|v| v[0]).max().unwrap() + 1;
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .by_exponential_blocks()
                        .find_first(|&&x| x[0] == needle)
                        .is_none())
                });
            }

            #[bench]
            fn serial_find_missing(b: &mut Bencher) {
                let needle = HAYSTACK.iter().map(|v| v[0]).max().unwrap() + 1;
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_none()));
            }

            #[bench]
            fn parallel_find_any_common(b: &mut Bencher) {
                b.iter(|| {
                    assert!(HAYSTACK
                        .par_iter()
                        .find_any(|&&x| x[0] % 1000 == 999)
                        .is_some())
                });
            }

            #[bench]
            fn serial_find_common(b: &mut Bencher) {
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] % 1000 == 999).is_some()));
            }
        }
    };
}

make_tests!(1, size1);
// make_tests!(64, size64);
// make_tests!(256, size256);
