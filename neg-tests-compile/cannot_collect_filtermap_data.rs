extern crate rayon;

use rayon::par_iter::*;

// zip requires data of exact size, but filter yields only bounded
// size, so check that we cannot apply it.

fn main() {
    let a: Vec<usize> = (0..1024).collect();
    let mut v = vec![];
    a.par_iter()
     .filter_map(|&x| Some(x as f32))
     .collect_into(&mut v); //~ ERROR no method
}
