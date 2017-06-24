#![deny(unused_must_use)]

extern crate rayon;

// Check that we are flagged for ignoring `must_use` parallel adaptors.

use rayon::prelude::*;

fn main() {
    let v: Vec<_> = (0..100).map(Some).collect();

    v.par_iter().chain(&v);                 //~ ERROR must be used
    v.par_iter().cloned();                  //~ ERROR must be used
    v.par_iter().enumerate();               //~ ERROR must be used
    v.par_iter().filter(|_| true);          //~ ERROR must be used
    v.par_iter().filter_map(|x| *x);        //~ ERROR must be used
    v.par_iter().flat_map(|x| *x);          //~ ERROR must be used
    v.par_iter().fold(|| 0, |x, _| x);      //~ ERROR must be used
    v.par_iter().fold_with(0, |x, _| x);    //~ ERROR must be used
    v.par_iter().inspect(|_| {});           //~ ERROR must be used
    v.par_iter().map(|x| x);                //~ ERROR must be used
    v.par_iter().map_with(0, |_, x| x);     //~ ERROR must be used
    v.par_iter().rev();                     //~ ERROR must be used
    v.par_iter().skip(1);                   //~ ERROR must be used
    v.par_iter().take(1);                   //~ ERROR must be used
    v.par_iter().cloned().while_some();     //~ ERROR must be used
    v.par_iter().with_max_len(1);           //~ ERROR must be used
    v.par_iter().with_min_len(1);           //~ ERROR must be used
    v.par_iter().zip(&v);                   //~ ERROR must be used
}
