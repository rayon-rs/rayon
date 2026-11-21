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
            assert_eq!(count, limit.min(8));
        }
    });
}

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn bounded_bridge_clone_and_reconfigure() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();

    pool.install(|| {
        let bridge = (0..100).par_bridge().with_max_concurrency(1);
        assert_eq!(bridge.clone().count(), 100);
        assert_eq!(bridge.clone().fold(|| (), |(), _| ()).count(), 1);
        let bridge = bridge.with_max_concurrency(3);
        assert_eq!(bridge.clone().fold(|| (), |(), _| ()).count(), 3);
        assert_eq!(
            bridge.with_max_concurrency(1).fold(|| (), |(), _| ()).count(),
            1
        );
    });
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
        (0..100)
            .par_bridge()
            .with_max_concurrency(2)
            .find_any(|&i| i == 42),
        Some(42)
    );
}

#[test]
#[should_panic(expected = "max_concurrency must not be zero")]
fn zero_concurrency() {
    let _ = (0..10).par_bridge().with_max_concurrency(0);
}

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn bounded_bridge_nested_rayon() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();

    pool.install(|| {
        for limit in [1, 2, 4] {
            let mut items: Vec<_> = (0..1000)
                .map(|i| rayon::join(|| i, || i + 1))
                .par_bridge()
                .with_max_concurrency(limit)
                .map(|(a, b)| rayon::join(|| a, || b))
                .collect();
            items.sort_unstable();
            assert_eq!(items, (0..1000).map(|i| (i, i + 1)).collect::<Vec<_>>());
        }
    });
}

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn bounded_bridge_active_consumers() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();

    pool.install(|| {
        for limit in [1, 2, 3, 4] {
            let active = AtomicUsize::new(0);
            (0..1000)
                .par_bridge()
                .with_max_concurrency(limit)
                .for_each(|_| {
                    let count = active.fetch_add(1, Ordering::Relaxed) + 1;
                    assert!(count <= limit);
                    std::thread::yield_now();
                    active.fetch_sub(1, Ordering::Relaxed);
                });
            assert_eq!(active.load(Ordering::Relaxed), 0);
        }
    });
}
