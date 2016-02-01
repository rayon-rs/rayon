extern crate rayon;

use rayon::*;

fn main() {
    let result = initialize(Configuration::new().set_num_threads(0));

    assert_eq!(result, Err(InitError::NumberOfThreadsZero));
}
