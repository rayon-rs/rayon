extern crate rayon;

use rayon::*;

fn main() {
    let result1 = initialize(Configuration::new());
    assert_eq!(result1, Ok(()));
    let result2 = initialize(Configuration::new());
    assert_eq!(result2, Err(GlobalPoolAlreadyInitialized));
}
