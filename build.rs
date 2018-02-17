use std::env;
use std::process::Command;
use std::str;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RUSTC");

    let rustc = env::var_os("RUSTC").unwrap();
    let version = Command::new(rustc).arg("-V").output().unwrap().stdout;
    let version = str::from_utf8(&version).unwrap();
    if version.contains("nightly") || version.contains("dev") {
        // a few tests want to use nightly features
        println!("cargo:rustc-cfg=nightly");
    }
}
