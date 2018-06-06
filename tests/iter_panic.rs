extern crate rayon;

use rayon::prelude::*;

#[test]
#[should_panic(expected = "boom")]
fn iter_panic() {
    (0i32..2_000_000).into_par_iter().for_each(|i| {
        if i == 1_350_000 {
            panic!("boom")
        }
    });
}
