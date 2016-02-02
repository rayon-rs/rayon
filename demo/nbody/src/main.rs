 #![feature(augmented_assignments)]

extern crate cgmath;
extern crate docopt;
#[macro_use]
extern crate glium;
extern crate rand;
extern crate rayon;
extern crate rustc_serialize;
extern crate time;

use docopt::Docopt;
use rand::{SeedableRng, XorShiftRng};

mod nbody;
mod visualize;
use self::visualize::visualize_benchmarks;
use self::nbody::NBodyBenchmark;

const USAGE: &'static str = "
Usage: nbody bench [--no-par | --no-seq] [--visualize --bodies N --ticks N]
       nbody visualize [--mode MODE --bodies N]
       nbody (--help | --version)

Commands:
    bench              Run the benchmark and print the timings.
    visualize          Show the graphical visualizer.

Options:
    -h, --help         Show this message.
    --no-par           Skip parallel execution in the benchmark.
    --no-seq           Skip sequential execution in the benchmark.
    --mode MODE        Execution mode for the visualizer [default: par].
    --bodies N         Use N bodies [default: 4000].
    --ticks N          Simulate for N ticks [default: 100].
";

#[derive(Copy, Clone, RustcDecodable)]
pub enum ExecutionMode {
    Par,
    Seq,
}

#[derive(RustcDecodable)]
pub struct Args {
    cmd_bench: bool,
    cmd_visualize: bool,
    flag_no_par: bool,
    flag_no_seq: bool,
    flag_mode: ExecutionMode,
    flag_bodies: usize,
    flag_ticks: usize,
}

fn main() {
    let args: Args =
        Docopt::new(USAGE)
            .and_then(|d| d.decode())
            .unwrap_or_else(|e| e.exit());

    if args.cmd_bench {
        run_benchmarks(!args.flag_no_par, !args.flag_no_seq,
                       args.flag_bodies, args.flag_ticks);
    }

    if args.cmd_visualize {
        visualize_benchmarks(args.flag_bodies, args.flag_mode);
    }
}

fn run_benchmarks(run_par: bool, run_seq: bool, bodies: usize, ticks: usize) {
    let par_time = if run_par {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let par_start = time::precise_time_ns();

        for _ in 0..ticks {
            benchmark.tick_par();
        }

        let par_time = time::precise_time_ns() - par_start;
        println!("Parallel time   : {}", par_time);

        Some(par_time)
    } else {
        None
    };

    let seq_time = if run_seq {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let seq_start = time::precise_time_ns();

        for _ in 0..ticks {
            benchmark.tick_seq();
        }

        let seq_time = time::precise_time_ns() - seq_start;
        println!("Sequential time : {}", seq_time);

        Some(seq_time)
    } else {
        None
    };

    if let (Some(pt), Some(st)) = (par_time, seq_time) {
        println!("Parallel speedup: {}", (st as f32) / (pt as f32));
    }
}
