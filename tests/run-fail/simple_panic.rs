extern crate rayon_core;

use rayon_core::*;

// error-pattern:should panic

fn main() {
    join(|| {}, || panic!("should panic"));
}
