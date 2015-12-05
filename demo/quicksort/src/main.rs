#![allow(non_camel_case_types)]

extern crate rand;
extern crate rayon;
extern crate time;

use rand::{Rng, SeedableRng, XorShiftRng};

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
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let data: Vec<_> = (0..18*1024).map(|_| rng.next_u32()).collect();

    rayon::initialize();

    let par_time = {
        let mut data = data.clone();
        let par_start = time::precise_time_ns();
        quick_sort::<Parallel, u32>(&mut data);
        time::precise_time_ns() - par_start
    };

    let seq_time = {
        //let mut data = data.clone();
        let seq_start = time::precise_time_ns();
        //quick_sort::<Sequential, u32>(&mut data);
        time::precise_time_ns() - seq_start
    };

    println!("Parallel time   : {}", par_time);
    println!("Sequential time : {}", seq_time);
    println!("Parallel speedup: {}", (seq_time as f64) / (par_time as f64));
}
