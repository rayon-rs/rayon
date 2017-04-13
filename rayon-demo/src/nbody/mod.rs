use docopt::Docopt;
use rand::{SeedableRng, XorShiftRng};
use time;

#[cfg(test)]
mod bench;
mod nbody;
mod visualize;
use self::visualize::visualize_benchmarks;
use self::nbody::NBodyBenchmark;

const USAGE: &'static str = "
Usage: nbody bench [--mode MODE --bodies N --ticks N]
       nbody visualize [--mode MODE --bodies N]
       nbody --help

Physics simulation of multiple bodies alternatively attracing and
repelling one another. Visualizable with OpenGL.

Commands:
    bench              Run the benchmark and print the timings.
    visualize          Show the graphical visualizer.

Options:
    -h, --help         Show this message.
    --mode MODE        Execution mode for the benchmark/visualizer.
    --bodies N         Use N bodies [default: 4000].
    --ticks N          Simulate for N ticks [default: 100].


Commands:
    bench              Run the benchmark and print the timings.
    visualize          Show the graphical visualizer.

Options:
    -h, --help         Show this message.
    --mode MODE        Execution mode for the benchmark/visualizer.
                       MODE can one of 'par', 'seq', or 'parreduce'.
    --bodies N         Use N bodies [default: 4000].
    --ticks N          Simulate for N ticks [default: 100].

Ported from the RiverTrail demo found at:
    https://github.com/IntelLabs/RiverTrail/tree/master/examples/nbody-webgl
";

#[derive(Copy, Clone, PartialEq, Eq, RustcDecodable)]
pub enum ExecutionMode {
    Par,
    ParReduce,
    Seq,
}

#[derive(RustcDecodable)]
pub struct Args {
    cmd_bench: bool,
    cmd_visualize: bool,
    flag_mode: Option<ExecutionMode>,
    flag_bodies: usize,
    flag_ticks: usize,
}

pub fn main(args: &[String]) {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(args).decode())
        .unwrap_or_else(|e| e.exit());

    if args.cmd_bench {
        run_benchmarks(args.flag_mode, args.flag_bodies, args.flag_ticks);
    }

    if args.cmd_visualize {
        visualize_benchmarks(
            args.flag_bodies,
            args.flag_mode.unwrap_or(ExecutionMode::Par),
        );
    }
}

fn run_benchmarks(mode: Option<ExecutionMode>, bodies: usize, ticks: usize) {
    let run_par = mode.map(|m| m == ExecutionMode::Par).unwrap_or(true);
    let run_par_reduce = mode.map(|m| m == ExecutionMode::ParReduce)
        .unwrap_or(true);
    let run_seq = mode.map(|m| m == ExecutionMode::Seq).unwrap_or(true);

    let par_time = if run_par {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let par_start = time::precise_time_ns();

        for _ in 0..ticks {
            benchmark.tick_par();
        }

        let par_time = time::precise_time_ns() - par_start;
        println!("Parallel time    : {} ns", par_time);

        Some(par_time)
    } else {
        None
    };

    let par_reduce_time = if run_par_reduce {
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let par_start = time::precise_time_ns();

        for _ in 0..ticks {
            benchmark.tick_par_reduce();
        }

        let par_time = time::precise_time_ns() - par_start;
        println!("ParReduce time   : {} ns", par_time);

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
        println!("Sequential time  : {} ns", seq_time);

        Some(seq_time)
    } else {
        None
    };

    if let (Some(pt), Some(st)) = (par_time, seq_time) {
        println!("Parallel speedup : {}", (st as f32) / (pt as f32));
    }

    if let (Some(pt), Some(st)) = (par_reduce_time, seq_time) {
        println!("ParReduce speedup: {}", (st as f32) / (pt as f32));
    }
}
