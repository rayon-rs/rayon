extern crate rayon;

use rayon::*;

fn main() {
    let result = Configuration::new().set_num_threads(0).initialize();

    assert_eq!(result, Err(InitResult::NumberOfThreadsZero));
}
