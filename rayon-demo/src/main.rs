#![cfg_attr(test, feature(test))]
#![feature(conservative_impl_trait)]

use std::env;
use std::io;
use std::io::prelude::*;
use std::process::exit;

mod matmul;
mod mergesort;
mod nbody;
mod quicksort;
mod sieve;
mod tsp;

// these are not "full-fledged" benchmarks yet,
// they only run with cargo bench
#[cfg(test)]
mod map_collect;
#[cfg(test)]
mod vec_collect;
#[cfg(test)]
mod factorial;
#[cfg(test)]
mod pythagoras;
#[cfg(test)]
mod fibonacci;
#[cfg(test)]
mod find;
#[cfg(test)]
mod join_microbench;

extern crate rayon; // all
extern crate docopt; // all
extern crate cgmath; // nbody
#[macro_use]
extern crate glium; // nbody
extern crate rand; // nbody
extern crate rustc_serialize; // nbody
extern crate time; // nbody, sieve
extern crate odds; // sieve
#[cfg(test)]
extern crate num; // factorial
#[macro_use]
extern crate lazy_static; // find
extern crate fixedbitset; // tsp
extern crate regex; // tsp

#[cfg(test)]
extern crate test;

const USAGE: &'static str = "
Usage: rayon-demo bench
       rayon-demo <demo-name> [ options ]
       rayon-demo --help

A collection of different benchmarks of Rayon. You can run the full
benchmark suite by executing `cargo bench` or `rayon-demo bench`.

Alternatively, you can run individual benchmarks by running
`rayon-demo foo`, where `foo` is the name of a benchmark. Each
benchmark has its own options and modes, so try `rayon-demo foo
--help`.

Benchmarks:

  - nbody: A physics simulation of multiple bodies attracting and repelling
           one another.
  - sieve: Finding primes using a Sieve of Eratosthenes.
  - matmul: Parallel matrix multiplication.
  - mergesort: Parallel mergesort.
  - quicksort: Parallel quicksort.
  - tsp: Traveling salesman problem solver (sample data sets in `data/tsp`).
";

fn usage() -> ! {
    let _ = writeln!(&mut io::stderr(), "{}", USAGE);
    exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        usage();
    }

    let bench_name = &args[1];
    match &bench_name[..] {
        "matmul" => matmul::main(&args[1..]),
        "mergesort" => mergesort::main(&args[1..]),
        "nbody" => nbody::main(&args[1..]),
        "quicksort" => quicksort::main(&args[1..]),
        "sieve" => sieve::main(&args[1..]),
        "tsp" => tsp::main(&args[1..]),
        _ => usage(),
    }
}
