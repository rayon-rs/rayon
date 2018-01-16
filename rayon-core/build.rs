extern crate gcc;

use std::env;

// We need a build script to use `link = "rayon-core"`.  But we're not
// *actually* linking to anything, just making sure that we're the only
// rayon-core in use.
fn main() {
    let target = env::var("TARGET").unwrap();

    let mut cfg = gcc::Build::new();

    if target.contains("windows") {
        cfg.file("src/fiber/windows.c");
    }

    cfg.compile("librayon_core_c.a");

    // we don't need to rebuild for anything else
    //println!("cargo:rerun-if-changed=build.rs");
}
