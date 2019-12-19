#![allow(non_camel_case_types)]

const USAGE: &str = "
Usage: quicksort bench [options]
       quicksort --help

Parallel quicksort. Only the main recursive step is parallelized.

Commands:
    bench              Run the benchmark in different modes and print the timings.

Options:
    --size N           Number of 32-bit words to sort [default: 250000000] (1GB)
    --par-only         Skip the sequential sort.
    -h, --help         Show this message.
";

#[derive(serde::Deserialize)]
pub struct Args {
    cmd_bench: bool,
    flag_size: usize,
    flag_par_only: bool,
}

use docopt::Docopt;
use rand::distributions::Standard;
use rand::Rng;
use std::time::Instant;

pub trait Joiner {
    fn is_parallel() -> bool;
    fn join<A, R_A, B, R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
    where
        A: FnOnce() -> R_A + Send,
        B: FnOnce() -> R_B + Send,
        R_A: Send,
        R_B: Send;
}

pub struct Parallel;
impl Joiner for Parallel {
    #[inline]
    fn is_parallel() -> bool {
        true
    }
    #[inline]
    fn join<A, R_A, B, R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
    where
        A: FnOnce() -> R_A + Send,
        B: FnOnce() -> R_B + Send,
        R_A: Send,
        R_B: Send,
    {
        rayon::join(oper_a, oper_b)
    }
}

struct Sequential;
impl Joiner for Sequential {
    #[inline]
    fn is_parallel() -> bool {
        false
    }
    #[inline]
    fn join<A, R_A, B, R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
    where
        A: FnOnce() -> R_A + Send,
        B: FnOnce() -> R_B + Send,
        R_A: Send,
        R_B: Send,
    {
        let a = oper_a();
        let b = oper_b();
        (a, b)
    }
}

pub fn quick_sort<J: Joiner, T: PartialOrd + Send>(v: &mut [T]) {
    if v.len() <= 1 {
        return;
    }

    if J::is_parallel() && v.len() <= 5 * 1024 {
        return quick_sort::<Sequential, T>(v);
    }

    let mid = partition(v);
    let (lo, hi) = v.split_at_mut(mid);
    J::join(|| quick_sort::<J, T>(lo), || quick_sort::<J, T>(hi));
}

fn partition<T: PartialOrd + Send>(v: &mut [T]) -> usize {
    let pivot = v.len() - 1;
    let mut i = 0;
    for j in 0..pivot {
        if v[j] <= v[pivot] {
            v.swap(i, j);
            i += 1;
        }
    }
    v.swap(i, pivot);
    i
}

pub fn is_sorted<T: Send + Ord>(v: &[T]) -> bool {
    (1..v.len()).all(|i| v[i - 1] <= v[i])
}

fn default_vec(n: usize) -> Vec<u32> {
    let rng = crate::seeded_rng();
    rng.sample_iter(&Standard).take(n).collect()
}

fn timed_sort<F: FnOnce(&mut [u32])>(n: usize, f: F, name: &str) -> u64 {
    let mut v = default_vec(n);

    let start = Instant::now();
    f(&mut v[..]);
    let dur = Instant::now() - start;
    let nanos = u64::from(dur.subsec_nanos()) + dur.as_secs() * 1_000_000_000u64;
    println!("{}: sorted {} ints: {} s", name, n, nanos as f32 / 1e9f32);

    // Check correctness
    assert!(is_sorted(&v[..]));

    nanos
}

pub fn main(args: &[String]) {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(args).deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.cmd_bench {
        if args.flag_par_only {
            timed_sort(args.flag_size, quick_sort::<Parallel, u32>, "par");
        } else {
            let seq = timed_sort(args.flag_size, quick_sort::<Sequential, u32>, "seq");
            let par = timed_sort(args.flag_size, quick_sort::<Parallel, u32>, "par");
            let speedup = seq as f64 / par as f64;
            println!("speedup: {:.2}x", speedup);
        }
    }
}

#[cfg(test)]
mod bench;
