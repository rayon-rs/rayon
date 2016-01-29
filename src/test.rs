#![cfg(test)]

extern crate compiletest_rs as compiletest;

use api::*;
use rand::{Rng, SeedableRng, XorShiftRng};
use std::path::PathBuf;

fn quick_sort<T:PartialOrd+Send>(v: &mut [T]) {
    if v.len() <= 1 {
        return;
    }

    let mid = partition(v);
    let (lo, hi) = v.split_at_mut(mid);
    join(|| quick_sort(lo),
         || quick_sort(hi));
}

fn partition<T:PartialOrd+Send>(v: &mut [T]) -> usize {
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
    let mut data: Vec<_> = (0..6*1024).map(|_| rng.next_u32()).collect();

    let result = Configuration::new().set_bench().initialize();

    match result {
        InitResult::InitOk => {
            quick_sort(&mut data);

            let mut sorted_data = data.clone();
            sorted_data.sort();

            assert_eq!(data, sorted_data);
        },
        InitResult::NumberOfThreadsZero => panic!("expected InitOk, but got NumberOfThreadsZero"),
        InitResult::NumberOfThreadsNotEqual => panic!("expected InitOk, but got NumberOfThreadsNotEqual")
    }
}

#[test]
fn sort_in_pool() {
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let mut data: Vec<_> = (0..12*1024).map(|_| rng.next_u32()).collect();

    let result = ThreadPool::new(None);

    match result {
        Ok(pool) => {
            pool.install(|| {
                quick_sort(&mut data);
            });

            let mut sorted_data = data.clone();
            sorted_data.sort();

            assert_eq!(data, sorted_data);
        },
        Err(_) => panic!("expected Ok() but got Err()")
    }
}

#[test]
fn error_in_pool() {
    let result = ThreadPool::new(Some(0));

    match result {
        Ok(_) => panic!("expected Err(), but got Ok()"),
        Err(error) => assert_eq!(error, InitResult::NumberOfThreadsZero)
    }
}

#[test]
fn negative_tests_compile_fail() {
    let mode = "compile-fail";
    let mut config = compiletest::default_config();
    let cfg_mode = mode.parse().ok().expect("Invalid mode");

    config.mode = cfg_mode;
    config.src_base = PathBuf::from("neg-tests-compile");
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
}

#[test]
fn positive_test_run_pass() {
    let mode = "run-pass";
    let mut config = compiletest::default_config();
    let cfg_mode = mode.parse().ok().expect("Invalid mode");

    config.mode = cfg_mode;
    config.src_base = PathBuf::from("pos-tests-run");
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
}
