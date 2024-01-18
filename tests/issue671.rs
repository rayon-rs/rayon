#![type_length_limit = "500000"]

use rayon::prelude::*;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn type_length_limit() {
    let _ = Vec::<Result<(), ()>>::new()
        .into_par_iter()
        .map(|x| x)
        .map(|x| x)
        .map(|x| x)
        .map(|x| x)
        .map(|x| x)
        .map(|x| x)
        .collect::<Result<(), ()>>();
}
