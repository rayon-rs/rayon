extern crate rayon_core;

use rayon_core::ThreadPoolBuilder;

#[test]
fn init_zero_threads() {
    ThreadPoolBuilder::new()
        .num_threads(0)
        .build_global()
        .unwrap();
}
