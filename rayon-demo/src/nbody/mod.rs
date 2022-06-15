use clap::{Parser, Subcommand, ValueEnum};
use std::time::Instant;

#[cfg(test)]
mod bench;
mod nbody;
mod visualize;
use self::nbody::NBodyBenchmark;
use self::visualize::visualize_benchmarks;

const ABOUT: &str = "
Physics simulation of multiple bodies alternatively attracting and
repelling one another. Visualizable with OpenGL.

Ported from the RiverTrail demo found at:
    https://github.com/IntelLabs/RiverTrail/tree/master/examples/nbody-webgl
";

#[derive(Subcommand)]
enum Commands {
    /// Run the benchmark and print the timings
    Bench,
    /// Show the graphical visualizer
    Visualize,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum ExecutionMode {
    Par,
    ParReduce,
    Seq,
}

#[derive(Parser)]
#[clap(about = ABOUT)]
pub struct Args {
    #[clap(subcommand)]
    command: Commands,

    /// Use N bodies
    #[clap(long, default_value_t = 4000)]
    bodies: usize,

    /// Simulate for N ticks
    #[clap(long, default_value_t = 100)]
    ticks: usize,

    /// Execution mode for the benchmark/visualizer
    #[clap(value_enum, long)]
    mode: Option<ExecutionMode>,
}

pub fn main(args: &[String]) {
    let args: Args = Parser::parse_from(args);

    match args.command {
        Commands::Bench => {
            run_benchmarks(args.mode, args.bodies, args.ticks);
        }
        Commands::Visualize => {
            visualize_benchmarks(args.bodies, args.mode.unwrap_or(ExecutionMode::Par));
        }
    }
}

fn run_benchmarks(mode: Option<ExecutionMode>, bodies: usize, ticks: usize) {
    let run_par = mode.map(|m| m == ExecutionMode::Par).unwrap_or(true);
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
        println!("Parallel time    : {} ns", par_time);

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
        println!("ParReduce time   : {} ns", par_time);

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
