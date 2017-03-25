#![cfg(test)]

extern crate compiletest_rs as compiletest;

use configuration::*;
use thread_pool::*;

#[test]
fn worker_thread_index() {
    let pool = ThreadPool::new(Configuration::new().set_num_threads(22)).unwrap();
    assert_eq!(pool.num_threads(), 22);
    assert_eq!(pool.current_thread_index(), None);
    let index = pool.install(|| pool.current_thread_index().unwrap());
    assert!(index < 22);
}
