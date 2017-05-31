#![cfg(test)]

use std::iter;
use std::sync::atomic::{AtomicUsize, Ordering};
use prelude::*;

#[test]
fn basic() {
    let x = vec![1, 2, 3, 4, 5];
    let y: Vec<u32> = x.iter().into_par_pipeline().map(|&i| i + 1).collect();
    let z: Vec<u32> = x.iter().map(|&i| i + 1).collect();
    assert_eq!(y, z);
}

#[test]
fn no_deadlock() {
    /// Test that, even with only 1 thread, we are able to handle an
    /// infinite sequential iterator (because it will eventually
    /// become `full`).
    let tp = ::ThreadPool::new(::Configuration::new().num_threads(1)).unwrap();
    tp.install(|| {
        let counter = AtomicUsize::new(0);
        assert_eq!(Some(22),
                   iter::repeat(22)
                   .into_par_pipeline()
                   .find_any(|_| {
                       let v = counter.fetch_add(1, Ordering::SeqCst);
                       v > 1024
                   }));
    });
}

#[test]
fn not_send_in_scope() {
    ::scope(|s| {
        use std::rc::Rc;

        /// A version of `iter::repeat` that is not `Send`.
        struct NotSend {
            r: Rc<i32>,
        }

        impl Iterator for NotSend {
            type Item = i32;

            fn next(&mut self) -> Option<i32> {
                Some(*self.r)
            }
        }

        let iter = NotSend {
            r: Rc::new(22),
        };

        iter.into_par_pipeline_scoped(s)
            .find_first(|&x| x == 22);
    });
}
