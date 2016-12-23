#![cfg(test)]

extern crate compiletest_rs as compiletest;

use configuration::*;
use std::path::PathBuf;
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
