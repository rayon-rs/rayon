extern crate rayon;

use rayon::*;

fn main() {
    initialize(Configuration::new().set_num_threads(0)).unwrap();
}
