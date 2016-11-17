extern crate rayon;

use rayon::*;
use rayon::prelude::*;

// error-pattern:boom

fn main() {
    (0i32..2_000_000).into_par_iter().weight_max().for_each(|i| if i == 1_350_000 { panic!("boom") });
}
