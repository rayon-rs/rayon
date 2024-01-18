use rayon::prelude::*;
use std::iter::once_with;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const N: usize = 100_000;

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn par_bridge_recursion() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(10)
        .build()
        .unwrap();

    let seq: Vec<_> = (0..N).map(|i| (i, i.to_string())).collect();

    pool.broadcast(|_| {
        let mut par: Vec<_> = (0..N)
            .into_par_iter()
            .flat_map(|i| {
                once_with(move || {
                    // Using rayon within the serial iterator creates an opportunity for
                    // work-stealing to make par_bridge's mutex accidentally recursive.
                    rayon::join(move || i, move || i.to_string())
                })
                .par_bridge()
            })
            .collect();
        par.par_sort_unstable();
        assert_eq!(seq, par);
    });
}
