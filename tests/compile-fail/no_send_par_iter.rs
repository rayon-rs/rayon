extern crate rayon;

// Check that `!Send` types fail early.

use rayon::prelude::*;
use std::ptr::null;

#[derive(Copy, Clone)]
struct NoSend(*const ());

unsafe impl Sync for NoSend {}

fn main() {
    let x = Some(NoSend(null()));

    x.par_iter()
        .map(|&x| x) //~ ERROR Send` is not satisfied
        .count(); //~ ERROR Send` is not satisfied

    x.par_iter()
        .filter_map(|&x| Some(x)) //~ ERROR Send` is not satisfied
        .count(); //~ ERROR Send` is not satisfied

    x.par_iter()
        .cloned() //~ ERROR Send` is not satisfied
        .count(); //~ ERROR Send` is not satisfied
}
