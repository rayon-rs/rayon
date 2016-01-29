extern crate rayon;

use rayon::*;

fn main() {
    let result1 = Configuration::new().set_num_threads(2).initialize();
    assert_eq!(result1, InitResult::InitOk);

    let result2 = Configuration::new().set_num_threads(2).initialize();
    assert_eq!(result2, InitResult::InitOk);

    let result3 = Configuration::new().set_num_threads(3).initialize();
    assert_eq!(result3, InitResult::NumberOfThreadsNotEqual);
}
