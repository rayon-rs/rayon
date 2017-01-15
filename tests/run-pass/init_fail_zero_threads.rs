extern crate rayon_core;

use rayon_core::*;

fn main() {
    let result = initialize(Configuration::new().set_num_threads(0));

    assert_eq!(result, Err(InitError::NumberOfThreadsZero));
}
