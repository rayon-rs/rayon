extern crate rayon;

use rayon::*;

fn main() {
    ThreadPoolBuilder::new().num_threads(0).build_global().unwrap();
}
