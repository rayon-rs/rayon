use std::collections::HashSet;

use rayon::prelude::*;
use rayon::*;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn named_threads() {
    ThreadPoolBuilder::new()
        .thread_name(|i| format!("hello-name-test-{}", i))
        .build_global()
        .unwrap();

    const N: usize = 10000;

    let thread_names = (0..N)
        .into_par_iter()
        .flat_map(|_| ::std::thread::current().name().map(str::to_owned))
        .collect::<HashSet<String>>();

    let all_contains_name = thread_names
        .iter()
        .all(|name| name.starts_with("hello-name-test-"));
    assert!(all_contains_name);
}
