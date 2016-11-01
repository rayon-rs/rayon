/// Simple benchmarks of `find()` performance

macro_rules! make_tests {
    ($n:expr, $m:ident) => {
        mod $m {
            use rayon::prelude::*;
            use test::Bencher;
            use rand::{Rng, SeedableRng, XorShiftRng};

            lazy_static! {
                static ref HAYSTACK: Vec<[u32; $n]> = {
                    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
                    (0..10_000_000).map(|_| {
                        let mut result: [u32; $n] = [0; $n];
                        result[0] = rng.next_u32();
                        result
                    }).collect()
                };
            }

            #[bench]
            fn parallel_find_first(b: &mut Bencher) {
                let needle = HAYSTACK[0][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn serial_find_first(b: &mut Bencher) {
                let needle = HAYSTACK[0][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_last(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len()-1][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn serial_find_last(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len()-1][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_middle(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3 * 2][0];
                b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn serial_find_middle(b: &mut Bencher) {
                let needle = HAYSTACK[HAYSTACK.len() / 3 * 2][0];
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_some()));
            }

            #[bench]
            fn parallel_find_missing(b: &mut Bencher) {
                let needle = HAYSTACK.iter().map(|v| v[0]).max().unwrap() + 1;
                b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x[0] == needle).is_none()));
            }

            #[bench]
            fn serial_find_missing(b: &mut Bencher) {
                let needle = HAYSTACK.iter().map(|v| v[0]).max().unwrap() + 1;
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] == needle).is_none()));
            }

            #[bench]
            fn parallel_find_common(b: &mut Bencher) {
                b.iter(|| assert!(HAYSTACK.par_iter().find(|&&x| x[0] % 1000 == 999).is_some()));
            }

            #[bench]
            fn serial_find_common(b: &mut Bencher) {
                b.iter(|| assert!(HAYSTACK.iter().find(|&&x| x[0] % 1000 == 999).is_some()));
            }
        }
    }
}

make_tests!(1, size1);
make_tests!(64, size64);
make_tests!(256, size256);
