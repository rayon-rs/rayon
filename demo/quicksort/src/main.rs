#![allow(non_camel_case_types)]

extern crate rand;
extern crate rayon;
extern crate time;

use rand::{Rng, SeedableRng, XorShiftRng};
use std::env;
use std::str::FromStr;

trait Joiner {
    fn join<A,R_A,B,R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
        where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send;
}

struct Parallel;
impl Joiner for Parallel {
    fn join<A,R_A,B,R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
        where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send
    {
        rayon::join(oper_a, oper_b)
    }
}

struct Sequential;
impl Joiner for Sequential {
    fn join<A,R_A,B,R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
        where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send
    {
        let a = oper_a();
        let b = oper_b();
        (a, b)
    }
}

fn quick_sort<J:Joiner, T:PartialOrd+Send>(v: &mut [T]) {
    if v.len() <= 1 {
        return;
    }

    let mid = partition(v);
    let (lo, hi) = v.split_at_mut(mid);
    J::join(|| quick_sort::<J,T>(lo),
            || quick_sort::<J,T>(hi));
}

fn partition<T:PartialOrd+Send>(v: &mut [T]) -> usize {
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

fn main() {
    let mut kilo = 24;
    let mut run_par = true;
    let mut run_seq = true;

    let args = env::args();
    for arg in args.skip(1) {
        if arg == "--no-par" {
            run_par = false;
        } else if arg == "--no-seq" {
            run_seq = false;
        } else {
            match usize::from_str(&arg) {
                Ok(v) => kilo = v,
                Err(_) => {
                    println!("invalid argument `{}`, expected integer", arg);
                }
            }
        }
    }

    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let data: Vec<_> = (0..kilo*1024).map(|_| rng.next_u32()).collect();

    rayon::initialize();

    let par_time = if run_par {
        let mut data = data.clone();
        let par_start = time::precise_time_ns();
        quick_sort::<Parallel, u32>(&mut data);
        time::precise_time_ns() - par_start
    } else {
        0
    };

    let seq_time = if run_seq {
        let mut data = data.clone();
        let seq_start = time::precise_time_ns();
        quick_sort::<Sequential, u32>(&mut data);
        time::precise_time_ns() - seq_start
    } else {
        0
    };

    println!("Array length    : {}K 32-bit integers", kilo);
    if run_par { println!("Parallel time   : {}", par_time); }
    if run_seq { println!("Sequential time : {}", seq_time); }
    if run_par && run_seq {
        println!("Parallel speedup: {}", (seq_time as f64) / (par_time as f64));
    }
}
