use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn bounded_concurrency() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();

    pool.install(|| {
        for limit in [1, 2, 3, 4, 8, 16, usize::MAX] {
            // fold initializes one accumulator for each leaf consumer, so this
            // bounds the number of consumers without relying on thread timing.
            let consumers = AtomicUsize::new(0);
            let mut items: Vec<_> = (0..1000)
                .par_bridge()
                .with_max_concurrency(limit)
                .fold(
                    || {
                        consumers.fetch_add(1, Ordering::Relaxed);
                        Vec::new()
                    },
                    |mut items, item| {
                        items.push(item);
                        items
                    },
                )
                .flatten_iter()
                .collect();
            items.sort_unstable();
            assert_eq!(items, (0..1000).collect::<Vec<_>>());
            let count = consumers.load(Ordering::Relaxed);
            assert!((1..=limit.min(8)).contains(&count));
        }
    });
}

#[test]
fn bounded_bridge_clone_and_reconfigure() {
    let bridge = (0..100).par_bridge().with_max_concurrency(4);
    assert_eq!(bridge.clone().count(), 100);
    let consumers = bridge
        .with_max_concurrency(1)
        .fold(|| (), |(), _| ())
        .count();
    assert_eq!(consumers, 1);
}

#[test]
fn bounded_bridge_empty_and_short_circuit() {
    assert_eq!(
        std::iter::empty::<usize>()
            .par_bridge()
            .with_max_concurrency(2)
            .count(),
        0
    );
    assert_eq!(
        (0..100).par_bridge().with_max_concurrency(2).find_any(|&i| i == 42),
        Some(42)
    );
}

#[test]
#[should_panic(expected = "max_concurrency must not be zero")]
fn zero_concurrency() {
    let _ = (0..10).par_bridge().with_max_concurrency(0);
}
