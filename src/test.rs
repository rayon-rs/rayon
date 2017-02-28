#![cfg(test)]

extern crate compiletest_rs as compiletest;

use std::path::PathBuf;

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
