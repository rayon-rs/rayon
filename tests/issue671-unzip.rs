#![type_length_limit = "10000"]

use rayon::prelude::*;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn type_length_limit() {
    let input = vec![1, 2, 3, 4, 5];
    let (indexes, (squares, cubes)): (Vec<_>, (Vec<_>, Vec<_>)) = input
        .par_iter()
        .map(|x| (x * x, x * x * x))
        .enumerate()
        .unzip();

    drop(indexes);
    drop(squares);
    drop(cubes);
}
