extern crate rayon;

use rayon::*;

fn main() {
    let result1 = initialize(Configuration::new());
    assert_eq!(result1.unwrap(), ());
    let err = initialize(Configuration::new()).unwrap_err();
    assert!(err.description() == "The global thread pool has already been initialized.");
}
