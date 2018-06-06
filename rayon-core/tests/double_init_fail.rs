extern crate rayon_core;

use rayon_core::ThreadPoolBuilder;
use std::error::Error;

#[test]
fn double_init_fail() {
    let result1 = ThreadPoolBuilder::new().build_global();
    assert_eq!(result1.unwrap(), ());
    let err = ThreadPoolBuilder::new().build_global().unwrap_err();
    assert!(err.description() == "The global thread pool has already been initialized.");
}
