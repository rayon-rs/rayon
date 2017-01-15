extern crate rayon_core;

// Check that we can't use the par-iter API to access contents of a `Cell`.

use rayon_core::prelude::*;
use std::cell::Cell;

fn main() {
    let c = Cell::new(42_i32);
    (0_i32..1024).into_par_iter()
             .map(|_| c.get()) //~ ERROR Sync` is not satisfied
             .min();
}
