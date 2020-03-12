use docopt::Docopt;
use std::time::Instant;

#[cfg(test)]
mod bench;
mod nbody;
mod visualize;
use self::nbody::NBodyBenchmark;
use self::visualize::visualize_benchmarks;

const USAGE: &str = "
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

#[derive(Copy, Clone, PartialEq, Eq, serde::Deserialize)]
pub enum ExecutionMode {
    Par,
    ParSched,
    ParBridge,
    ParReduce,
    Seq,
}

#[derive(serde::Deserialize)]
pub struct Args {
    cmd_bench: bool,
    cmd_visualize: bool,
    flag_mode: Option<ExecutionMode>,
    flag_bodies: usize,
    flag_ticks: usize,
}

pub fn main(args: &[String]) {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(args).deserialize())
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
    let run_par_schedule = mode.map(|m| m == ExecutionMode::ParSched).unwrap_or(true);
    let run_par_bridge = mode.map(|m| m == ExecutionMode::ParBridge).unwrap_or(true);
    let run_par_reduce = mode.map(|m| m == ExecutionMode::ParReduce).unwrap_or(true);
    let run_seq = mode.map(|m| m == ExecutionMode::Seq).unwrap_or(true);

    let par_time = if run_par {
        let mut rng = crate::seeded_rng();
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let par_start = Instant::now();

        for _ in 0..ticks {
            benchmark.tick_par();
        }

        let par_time = par_start.elapsed().as_nanos();
        println!("Parallel time\t: {} ns", par_time);

        Some(par_time)
    } else {
        None
    };

    let par_schedule_time = if run_par_schedule {
        let mut rng = crate::seeded_rng();
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let par_start = Instant::now();

        for _ in 0..ticks {
            benchmark.tick_par_schedule();
        }

        let par_time = par_start.elapsed().as_nanos();
        println!("ParSched time\t: {} ns", par_time);

        Some(par_time)
    } else {
        None
    };

    let par_bridge_time = if run_par_bridge {
        let mut rng = crate::seeded_rng();
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let par_start = Instant::now();

        for _ in 0..ticks {
            benchmark.tick_par_bridge();
        }

        let par_time = par_start.elapsed().as_nanos();
        println!("ParBridge time\t: {} ns", par_time);

        Some(par_time)
    } else {
        None
    };

    let par_reduce_time = if run_par_reduce {
        let mut rng = crate::seeded_rng();
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let par_start = Instant::now();

        for _ in 0..ticks {
            benchmark.tick_par_reduce();
        }

        let par_time = par_start.elapsed().as_nanos();
        println!("ParReduce time\t: {} ns", par_time);

        Some(par_time)
    } else {
        None
    };

    let seq_time = if run_seq {
        let mut rng = crate::seeded_rng();
        let mut benchmark = NBodyBenchmark::new(bodies, &mut rng);
        let seq_start = Instant::now();

        for _ in 0..ticks {
            benchmark.tick_seq();
        }

        let seq_time = seq_start.elapsed().as_nanos();
        println!("Sequential time\t: {} ns", seq_time);

        Some(seq_time)
    } else {
        None
    };

    if let (Some(pt), Some(st)) = (par_time, seq_time) {
        println!("Parallel speedup\t: {}", (st as f32) / (pt as f32));
    }

    if let (Some(pt), Some(st)) = (par_schedule_time, seq_time) {
        println!("ParSched speedup\t: {}", (st as f32) / (pt as f32));
    }

    if let (Some(pt), Some(st)) = (par_bridge_time, seq_time) {
        println!("ParBridge speedup\t: {}", (st as f32) / (pt as f32));
    }

    if let (Some(pt), Some(st)) = (par_reduce_time, seq_time) {
        println!("ParReduce speedup\t: {}", (st as f32) / (pt as f32));
    }
}
