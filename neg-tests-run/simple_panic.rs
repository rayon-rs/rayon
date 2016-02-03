extern crate rayon;

use rayon::*;

// error-pattern:should panic

fn main() {
    join(|| {}, || panic!("should panic"));
}
