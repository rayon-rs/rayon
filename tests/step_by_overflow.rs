use rayon::prelude::*;

#[test]
fn step_by_large_step_take_and_skip() {
    for len in [0, 1, 2, usize::MAX / 2 + 1, usize::MAX - 1, usize::MAX] {
        for step in [
            usize::MAX / 2,
            usize::MAX / 2 + 1,
            usize::MAX - 1,
            usize::MAX,
        ] {
            for n in 0..=3 {
                let expected: Vec<_> = (0..len).step_by(step).take(n).collect();
                let actual: Vec<_> = (0..len).into_par_iter().step_by(step).take(n).collect();
                assert_eq!(actual, expected, "take: len={len}, step={step}, n={n}");

                let expected: Vec<_> = (0..len).step_by(step).skip(n).collect();
                let actual: Vec<_> = (0..len).into_par_iter().step_by(step).skip(n).collect();
                assert_eq!(actual, expected, "skip: len={len}, step={step}, n={n}");
            }
        }
    }
}
