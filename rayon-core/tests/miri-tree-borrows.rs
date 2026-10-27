//! These are regression tests for undefined behavior that was detected with Miri under its Tree
//! Borrows aliasing model. Being UB, there might not be any reliable way to detect the presence
//! nor absence of such bugs in normal execution. Miri is also generally too slow to run the entire
//! test suite, so these tests are more targeted.
//!
//! ```shell
//! MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-ignore-leaks -Zmiri-many-seeds=0..32" \
//!     cargo +nightly miri test --package rayon-core --test miri-tree-borrows
//! ```
//!
//! (It would be nice to test Stack Borrows too, but `crossbeam-epoch` isn't clean there.)

use rayon_core::ThreadPoolBuilder;

// Scope spawns must not hold any protected tags from its closures at the point where the
// `Latch::set` releases the outer scope to return. Otherwise, they may codegen with LLVM
// `dereferenceable` attributes on arguments that are left dangling, which is UB.

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn dangling_scope_spawn() {
    let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();

    let check = || {
        for _ in 0..10 {
            let mut values = [0_u32; 4];

            pool.in_place_scope(|scope| {
                for value in &mut values {
                    scope.spawn(move |_| *value += 1);
                }
            });

            assert_eq!(values, [1; 4]);
        }
    };

    // The `CountLatch` is different depending on whether the scope was called within the pool, and
    // although that shouldn't be material to the bug that was fixed, we'll try both.
    check();
    pool.install(check);
}

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn dangling_scope_fifo_spawn() {
    let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();

    let check = || {
        for _ in 0..10 {
            let mut values = [0_u32; 4];

            pool.in_place_scope_fifo(|scope| {
                for value in &mut values {
                    scope.spawn_fifo(move |_| *value += 1);
                }
            });

            assert_eq!(values, [1; 4]);
        }
    };

    check();
    pool.install(check);
}
