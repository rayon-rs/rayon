#![cfg(test)]

extern crate compiletest_rs as compiletest;

use configuration::*;
use std::error::Error;
use thread_pool::*;

#[test]
fn error_in_pool() {
    let result = ThreadPool::new(Configuration::new().set_num_threads(0));

    match result {
        Ok(_) => panic!("expected Err(), but got Ok()"),
        Err(error) => assert_eq!(error, InitError::NumberOfThreadsZero),
    }
}

#[test]
fn coerce_to_box_error() {
    // check that coercion succeeds
    let _: Box<Error> = From::from(InitError::NumberOfThreadsZero);
}

#[test]
fn worker_thread_index() {
    let pool = ThreadPool::new(Configuration::new().set_num_threads(22)).unwrap();
    assert_eq!(pool.num_threads(), 22);
    assert_eq!(pool.current_thread_index(), None);
    let index = pool.install(|| pool.current_thread_index().unwrap());
    assert!(index < 22);
}
