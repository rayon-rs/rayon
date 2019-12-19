/// Simple benchmarks of `find_any()` performance

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

            #[bench]
            fn parallel_find_first(b: &mut Bencher) {
                let needle = HAYSTACK[0][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn serial_find_first(b: &mut Bencher) {
                let needle = HAYSTACK[0][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_last(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() - 1][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn serial_find_last(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() - 1][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_middle(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3 * 2][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn serial_find_middle(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3 * 2][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_missing(b: &mut Bencher) {
                let needle = HAYSTACK.iter().map(|v| v[0]).max().unwrap() + 1;
                b.iter(|| assert!(HAYSTACK.par_iter().find_any(|&&x| x[0] == needle).is_none()));
            }

            #[bench]
            fn serial_find_missing(b: &mut Bencher) {
                let needle = HAYSTACK.iter().map(|v| v[0]).max().unwrap() + 1;
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_none()));
            }

            #[bench]
            fn parallel_find_common(b: &mut Bencher) {
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
