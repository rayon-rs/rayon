extern crate rayon;

use rayon::*;

fn main() {
    initialize(Configuration::new().num_threads(0)).unwrap();
}
