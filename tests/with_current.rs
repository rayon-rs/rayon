use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::thread;

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn with_current() {
    fn high_priority_work() {
        assert_eq!(thread::current().name(), Some("high-priority-thread"));
    }

    fn regular_work(_item: &()) {
        assert_eq!(thread::current().name(), None);
    }

    let items = vec![(); 128];

    let default_pool = ThreadPool::current();

    let high_priority_pool = ThreadPoolBuilder::new()
        .thread_name(|_| "high-priority-thread".to_owned())
        .build()
        .unwrap();

    high_priority_pool.in_place_scope(|scope| {
        scope.spawn(|_| high_priority_work());

        default_pool.with_current(|| {
            items.par_iter().for_each(|item| regular_work(item));
        })
    });
}
