use prelude::*;
use Configuration;
use scope::{scope};
use ThreadPool;
use super::Atomic;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

const THREADS: usize = 100;

#[test]
fn random_threads() {
    let mut counter = 0;
    let atomic = Arc::new(Atomic::new(move |data: usize| -> usize {
        let value = counter;
        counter += data;
        value
    }));

    let handles: Vec<_> = (0..THREADS)
        .map(|i| {
            let atomic = atomic.clone();
            thread::spawn(move || {
                atomic.invoke(i);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let sum = atomic.invoke(0);
    assert_eq!(sum, (0..THREADS).sum());
}

#[test]
fn random_threads_scope() {
    let mut counter = 0;

    {
        let atomic = &Atomic::new(|data| counter += data);
        invoke_atomic(atomic);
    }

    assert_eq!(counter, (0..THREADS).sum());
}

#[test]
fn random_threads_scope_multiple_registries() {
    let mut counter = 0;

    {
        let atomic = &Atomic::new(|data| counter += data);

        scope(|s| {
            s.spawn(move |_| {
                ThreadPool::new(Configuration::new()).unwrap().install(move || {
                    invoke_atomic(atomic);
                });
            });

            s.spawn(move |_| {
                ThreadPool::new(Configuration::new()).unwrap().install(move || {
                    invoke_atomic(atomic);
                });
            });
        });
    }

    let sum: usize = (0..THREADS).sum();
    assert_eq!(counter, sum * 2);
}

fn invoke_atomic<F>(atomic: &Atomic<F, usize, ()>)
    where F: FnMut(usize) -> () + Send
{
    scope(|s| {
        for i in 0..THREADS {
            s.spawn(move |_| atomic.invoke(i));
        }
    });
}

#[test]
fn build_hashmap_nicely() {
    const N: usize = 64_000;

    let mut hashmap = HashMap::new();
    (0..N)
        .into_par_iter()
        .map(|i| (i, i + 1))
        .for_each(atomically!(|(k, v)| {
            hashmap.insert(k, v);
        }));
    for i in 0..N {
        assert_eq!(hashmap[&i], i + 1);
    }
}
