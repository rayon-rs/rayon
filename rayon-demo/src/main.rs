#![cfg_attr(test, feature(test))]

use std::env;
use std::io;
use std::io::prelude::*;
use std::process::exit;

mod mergesort;
mod nbody;
mod sieve;

extern crate rayon; // all
extern crate docopt; // all
extern crate cgmath; // nbody
#[macro_use]
extern crate glium; // nbody
extern crate rand; // nbody
extern crate rustc_serialize; // nbody
extern crate time; // nbody, sieve
extern crate itertools; // sieve

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
  - mergesort: Parallel mergesort.
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
        "mergesort" => mergesort::main(&args[1..]),
        "nbody" => nbody::main(&args[1..]),
        "sieve" => sieve::main(&args[1..]),
        _ => usage()
    }
}
