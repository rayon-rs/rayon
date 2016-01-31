extern crate rayon;

use rayon::par_iter::*;

// zip requires data of exact size, but filter yields only bounded
// size, so check that we cannot apply it.

fn main() {
    let mut a: Vec<usize> = (0..1024).rev().collect();
    let b: Vec<usize> = (0..1024).collect();

    a.par_iter()
     .zip(b.par_iter().filter(|&&x| x > 3)); //~ ERROR E0277
}
