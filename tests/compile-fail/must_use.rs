#![deny(unused_must_use)]

extern crate rayon;

// Check that we are flagged for ignoring `must_use` parallel adaptors.

use rayon::prelude::*;

fn main() {
    let v: Vec<_> = (0..100).map(Some).collect();

    v.par_iter().chain(&v);                 //~ ERROR unused result
    v.par_iter().cloned();                  //~ ERROR unused result
    v.par_iter().enumerate();               //~ ERROR unused result
    v.par_iter().filter(|_| true);          //~ ERROR unused result
    v.par_iter().filter_map(|x| *x);        //~ ERROR unused result
    v.par_iter().flat_map(|x| *x);          //~ ERROR unused result
    v.par_iter().fold(|| 0, |x, _| x);      //~ ERROR unused result
    v.par_iter().fold_with(0, |x, _| x);    //~ ERROR unused result
    v.par_iter().inspect(|_| {});           //~ ERROR unused result
    v.par_iter().map(|x| x);                //~ ERROR unused result
    v.par_iter().map_with(0, |_, x| x);     //~ ERROR unused result
    v.par_iter().rev();                     //~ ERROR unused result
    v.par_iter().skip(1);                   //~ ERROR unused result
    v.par_iter().take(1);                   //~ ERROR unused result
    v.par_iter().cloned().while_some();     //~ ERROR unused result
    v.par_iter().with_max_len(1);           //~ ERROR unused result
    v.par_iter().with_min_len(1);           //~ ERROR unused result
    v.par_iter().zip(&v);                   //~ ERROR unused result
}
