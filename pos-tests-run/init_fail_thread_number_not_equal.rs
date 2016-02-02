extern crate rayon;

use rayon::*;

fn main() {
    let result1 = initialize(Configuration::new().set_num_threads(2));
    assert_eq!(result1, Ok(()));

    let result2 = initialize(Configuration::new().set_num_threads(2));
    assert_eq!(result2, Ok(()));

    let result3 = initialize(Configuration::new().set_num_threads(3));
    assert_eq!(result3, Err(InitError::GlobalPoolAlreadyInitialized));
}
