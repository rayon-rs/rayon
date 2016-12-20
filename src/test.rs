#![cfg(test)]

extern crate compiletest_rs as compiletest;

use api::*;
use rand::{Rng, SeedableRng, XorShiftRng};
use std::path::PathBuf;
use std::error::Error;

fn quick_sort<T: PartialOrd + Send>(v: &mut [T]) {
    if v.len() <= 1 {
        return;
    }

    let mid = partition(v);
    let (lo, hi) = v.split_at_mut(mid);
    join(|| quick_sort(lo), || quick_sort(hi));
}

fn partition<T: PartialOrd + Send>(v: &mut [T]) -> usize {
    let pivot = v.len() - 1;
    let mut i = 0;
    for j in 0..pivot {
        if v[j] <= v[pivot] {
            v.swap(i, j);
            i += 1;
        }
    }
    v.swap(i, pivot);
    i
}

#[test]
fn sort() {
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let mut data: Vec<_> = (0..6 * 1024).map(|_| rng.next_u32()).collect();

    let result = initialize(Configuration::new());

    match result {
        Ok(_) => {
            quick_sort(&mut data);

            let mut sorted_data = data.clone();
            sorted_data.sort();

            assert_eq!(data, sorted_data);
        }
        Err(e) => panic!("expected InitOk, but got {:?}", e),
    }
}

#[test]
fn sort_in_pool() {
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let mut data: Vec<_> = (0..12 * 1024).map(|_| rng.next_u32()).collect();

    let result = ThreadPool::new(Configuration::new());

    match result {
        Ok(pool) => {
            pool.install(|| {
                quick_sort(&mut data);
            });

            let mut sorted_data = data.clone();
            sorted_data.sort();

            assert_eq!(data, sorted_data);
        }
        Err(_) => panic!("expected Ok() but got Err()"),
    }
}

#[test]
fn error_in_pool() {
    let result = ThreadPool::new(Configuration::new().set_num_threads(0));

    match result {
        Ok(_) => panic!("expected Err(), but got Ok()"),
        Err(error) => assert_eq!(error, InitError::NumberOfThreadsZero),
    }
}

#[test]
fn negative_tests_compile_fail() {
    let mode = "compile-fail";
    let mut config = compiletest::default_config();
    let cfg_mode = mode.parse().ok().expect("Invalid mode");

    config.mode = cfg_mode;
    config.src_base = PathBuf::from("tests/compile-fail");
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
}

#[test]
#[cfg(feature = "unstable")]
fn negative_tests_compile_fail_unstable() {
    let mode = "compile-fail";
    let mut config = compiletest::default_config();
    let cfg_mode = mode.parse().ok().expect("Invalid mode");

    config.mode = cfg_mode;
    config.src_base = PathBuf::from("tests/compile-fail-unstable");
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
}

#[test]
fn negative_tests_run_fail() {
    let mode = "run-fail";
    let mut config = compiletest::default_config();
    let cfg_mode = mode.parse().ok().expect("Invalid mode");

    config.mode = cfg_mode;
    config.src_base = PathBuf::from("tests/run-fail");
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
}

#[test]
#[cfg(feature = "unstable")]
fn negative_tests_run_fail_unstable() {
    let mode = "run-fail";
    let mut config = compiletest::default_config();
    let cfg_mode = mode.parse().ok().expect("Invalid mode");

    config.mode = cfg_mode;
    config.src_base = PathBuf::from("tests/run-fail-unstable");
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
}

#[test]
fn positive_test_run_pass() {
    let mode = "run-pass";
    let mut config = compiletest::default_config();
    let cfg_mode = mode.parse().ok().expect("Invalid mode");

    config.mode = cfg_mode;
    config.src_base = PathBuf::from("tests/run-pass");
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
}

#[test]
#[cfg(feature = "unstable")]
fn positive_test_run_pass_unstable() {
    let mode = "run-pass";
    let mut config = compiletest::default_config();
    let cfg_mode = mode.parse().ok().expect("Invalid mode");

    config.mode = cfg_mode;
    config.src_base = PathBuf::from("tests/run-pass-unstable");
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
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
