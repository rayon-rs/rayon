#![allow(non_camel_case_types)]

extern crate rand;
extern crate rayon;
extern crate time;

use rand::{Rng, SeedableRng, XorShiftRng};
use std::env;
use std::str::FromStr;

trait Joiner {
    fn is_parallel() -> bool;
    fn join<A,R_A,B,R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
        where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send,
              R_A: Send, R_B: Send;
}

struct Parallel;
impl Joiner for Parallel {
    #[inline]
    fn is_parallel() -> bool {
        true
    }
    #[inline]
    fn join<A,R_A,B,R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
        where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send,
              R_A: Send, R_B: Send
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
    fn join<A,R_A,B,R_B>(oper_a: A, oper_b: B) -> (R_A, R_B)
        where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send,
              R_A: Send, R_B: Send
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

    if J::is_parallel() && v.len() <= 5*1024 {
        return quick_sort::<Sequential, T>(v);
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
    let mut reps = 10;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--no-par" {
            run_par = false;
        } else if arg == "--no-seq" {
            run_seq = false;
        } else if arg == "--reps" {
            if let Some(reps_arg) = args.next() {
                match usize::from_str(&reps_arg) {
                    Ok(v) => reps = v,
                    Err(_) => {
                        println!("invalid argument `{}`, expected integer", reps_arg);
                    }
                }
            } else {
                println!("missing argument to --reps");
            }
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
    let mut par_time = 0;
    let mut seq_time = 0;

    rayon::initialize();

    for _ in 0 .. reps {
        let data: Vec<_> = (0..kilo*1024).map(|_| rng.next_u32()).collect();

        if run_par {
            let mut data = data.clone();
            let par_start = time::precise_time_ns();
            quick_sort::<Parallel, u32>(&mut data);
            par_time += time::precise_time_ns() - par_start;
        }

        if run_seq {
            let mut data = data.clone();
            let seq_start = time::precise_time_ns();
            quick_sort::<Sequential, u32>(&mut data);
            seq_time += time::precise_time_ns() - seq_start;
        }
    }

    println!("Array length    : {}K 32-bit integers", kilo);
    println!("Repetitions     : {}", reps);
    if run_par { println!("Parallel time   : {}", par_time); }
    if run_seq { println!("Sequential time : {}", seq_time); }
    if run_par && run_seq {
        println!("Parallel speedup: {}", (seq_time as f64) / (par_time as f64));
    }
}
