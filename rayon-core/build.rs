extern crate autocfg;

// We need a build script to use `link = "rayon-core"`.  But we're not
// *actually* linking to anything, just making sure that we're the only
// rayon-core in use.
fn main() {
    let ac = autocfg::new();
    ac.emit_path_cfg("std::future::Future", "has_future");

    // we don't need to rebuild for anything else
    autocfg::rerun_path("build.rs");
}
