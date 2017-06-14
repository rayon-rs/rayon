#![cfg(test)]

extern crate compiletest_rs as compiletest;

use std::path::PathBuf;

fn run_compiletest(mode: &str, path: &str) {
    let mut config = compiletest::default_config();
    config.mode = mode.parse().ok().expect("Invalid mode");
    config.src_base = PathBuf::from(path);
    config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());

    compiletest::run_tests(&config);
}

#[test]
fn negative_tests_compile_fail() {
    run_compiletest("compile-fail", "tests/compile-fail");
}

#[test]
#[cfg(rayon_unstable)]
fn negative_tests_compile_fail_unstable() {
    run_compiletest("compile-fail", "tests/compile-fail-unstable");
}

#[test]
fn negative_tests_run_fail() {
    run_compiletest("run-fail", "tests/run-fail");
}

#[test]
#[cfg(rayon_unstable)]
fn negative_tests_run_fail_unstable() {
    run_compiletest("run-fail", "tests/run-fail-unstable");
}

#[test]
fn positive_test_run_pass() {
    run_compiletest("run-pass", "tests/run-pass");
}

#[test]
#[cfg(rayon_unstable)]
fn positive_test_run_pass_unstable() {
    run_compiletest("run-pass", "tests/run-pass-unstable");
}
