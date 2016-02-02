extern crate rand;
extern crate rayon;
extern crate sdl;
extern crate time;

use rand::{SeedableRng, XorShiftRng};
use std::env;
use std::str::FromStr;

mod nbody;
mod visualize;
use self::visualize::visualize_benchmarks;
use self::nbody::NBodyBenchmark;

const DEFAULT_NUM_BODIES: usize = 4000;
const DEFAULT_TICKS: usize = 100;

fn main() {
    let mut num_bodies = DEFAULT_NUM_BODIES;
    let mut ticks = DEFAULT_TICKS;
    let mut run_par = true;
    let mut run_seq = true;
    let mut visualize = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--no-par" {
            run_par = false;
        } else if arg == "--no-seq" {
            run_seq = false;
        } else if arg == "--visualize" {
            visualize = true;
        } else if arg == "--ticks" {
            if let Some(ticks_arg) = args.next() {
                match usize::from_str(&ticks_arg) {
                    Ok(v) => ticks = v,
                    Err(_) => {
                        println!("invalid argument `{}`, expected integral number of ticks",
                                 ticks_arg);
                    }
                }
            } else {
                println!("missing argument to --ticks");
            }
        } else if arg == "--bodies" {
            if let Some(bodies_arg) = args.next() {
                match usize::from_str(&bodies_arg) {
                    Ok(v) => num_bodies = v,
                    Err(_) => {
                        println!("invalid argument `{}`, expected integral number of bodies", arg);
                    }
                }
            } else {
                println!("missing argument to --bodies");
            }
        } else if arg == "--help" {
            println!("Usage:");
            println!(" --no-par      skip parallel execution");
            println!(" --no-seq      skip sequential execution");
            println!(" --bodies N    use N bodies (default: {})", DEFAULT_NUM_BODIES);
            println!(" --ticks N     simulate for N ticks (default: {})", DEFAULT_TICKS);
            return;
        } else {
            println!("Illegal argument `{}`. Try --help.", arg);
            return;
        }
    }

    if visualize {
        visualize_benchmarks(num_bodies);
    } else {
        run_benchmarks(run_par, run_seq, num_bodies, ticks);
    }
}

fn run_benchmarks(run_par: bool,
                  run_seq: bool,
                  num_bodies: usize,
                  ticks: usize) {
    println!("Configuration:");
    if !run_par { println!("  --no-par"); }
    if !run_seq { println!("  --no-seq"); }
    println!("  --bodies {}", num_bodies);
    println!("  --ticks {}", ticks);

    let par_time = {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        let mut benchmark = NBodyBenchmark::new(num_bodies, &mut rng);
        let par_start = time::precise_time_ns();
        for _ in 0..ticks {
            benchmark.tick_par();
        }
        time::precise_time_ns() - par_start
    };

    let seq_time = {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        let mut benchmark = NBodyBenchmark::new(num_bodies, &mut rng);
        let seq_start = time::precise_time_ns();
        for _ in 0..ticks {
            benchmark.tick_seq();
        }
        time::precise_time_ns() - seq_start
    };

    if run_par { println!("Parallel time   : {}", par_time); }
    if run_seq { println!("Sequential time : {}", seq_time); }
    if run_par && run_seq {
        println!("Parallel speedup: {}", (seq_time as f64) / (par_time as f64));
    }
}
