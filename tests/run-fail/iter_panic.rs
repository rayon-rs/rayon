extern crate rayon_core;

use rayon_core::*;
use rayon_core::prelude::*;

// error-pattern:boom

fn main() {
    (0i32..2_000_000).into_par_iter().weight_max().for_each(|i| if i == 1_350_000 { panic!("boom") });
}
