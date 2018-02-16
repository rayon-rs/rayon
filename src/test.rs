#![cfg(test)]
#![cfg(not(all(windows, target_env = "gnu")))]

extern crate compiletest_rs as compiletest;

use std::path::PathBuf;

fn run_compiletest(mode: &str, path: &str) {
    let mut config = compiletest::Config::default();
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
