 #![feature(augmented_assignments)]

extern crate cgmath;
extern crate docopt;
#[macro_use]
extern crate glium;
extern crate pbr;
extern crate rand;
extern crate rayon;
extern crate rustc_serialize;
extern crate time;

use docopt::Docopt;
use pbr::ProgressBar;
use rand::{SeedableRng, XorShiftRng};
use std::fmt;

mod nbody;
mod visualize;
use self::visualize::visualize_benchmarks;
use self::nbody::NBodyBenchmark;

const USAGE: &'static str = "
Usage: nbody bench [--no-par | --no-seq] [--visualize --bodies N --ticks N]
       nbody visualize [--bodies N]
       nbody (--help | --version)

Commands:
    bench              Run the benchmark and print the timings.
    visualize          Show the graphical visualizer.

Options:
    -h, --help         Show this message.
    --no-par           Skip parallel execution.
    --no-seq           Skip sequential execution.
    --bodies N         Use N bodies [default: 4000].
    --ticks N          Simulate for N ticks [default: 100].
";

#[derive(RustcDecodable)]
struct Args {
    cmd_benchmark: bool,
    cmd_visualize: bool,
    flag_no_par: bool,
    flag_no_seq: bool,
    flag_bodies: usize,
    flag_ticks: usize,
}

impl fmt::Display for Args {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(writeln!(f, "Configuration:"));

        if self.cmd_benchmark   { try!(writeln!(f, "  bench")); }
        if self.cmd_visualize   { try!(writeln!(f, "  visualize")); }
        if self.flag_no_par     { try!(writeln!(f, "  --no-par")); }
        if self.flag_no_seq     { try!(writeln!(f, "  --no-seq")); }

        try!(writeln!(f, "  --bodies {}", self.flag_bodies));
        try!(writeln!(f, "  --ticks {}", self.flag_ticks));

        Ok(())
    }
}

fn main() {
    let args: Args =
        Docopt::new(USAGE)
            .and_then(|d| d.decode())
            .unwrap_or_else(|e| e.exit());

    if args.cmd_benchmark {
        run_benchmarks(!args.flag_no_par, !args.flag_no_seq,
                       args.flag_bodies, args.flag_ticks);
    }

    if args.cmd_visualize {
        visualize_benchmarks(args.flag_bodies);
    }
}

fn run_benchmarks(run_par: bool, run_seq: bool, bodies: usize, ticks: usize) {
    let par_time = if run_par {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let par_start = time::precise_time_ns();

        let mut progress = ProgressBar::new(ticks);
        for _ in 0..ticks {
            progress.inc();
            benchmark.tick_par();
        }
        progress.finish();

        Some(time::precise_time_ns() - par_start)
    } else {
        None
    };

    let seq_time = if run_seq {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let seq_start = time::precise_time_ns();

        let mut progress = ProgressBar::new(ticks);
        for _ in 0..ticks {
            progress.inc();
            benchmark.tick_seq();
        }
        progress.finish();

        Some(time::precise_time_ns() - seq_start)
    } else {
        None
    };

    if let Some(t) = par_time { println!("Parallel time   : {}", t); }
    if let Some(t) = seq_time { println!("Sequential time : {}", t); }

    if let (Some(pt), Some(st)) = (par_time, seq_time) {
        println!("Parallel speedup: {}", (st as f32) / (pt as f32));
    }
}
