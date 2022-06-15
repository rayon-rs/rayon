//! A solver for the Travelling Salesman Problem.
//!
//! Based on code developed at ETH by Christoph von Praun, Florian
//! Schneider, Nicholas Matsakis, and Thomas Gross.

use clap::{Parser, Subcommand};
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[cfg(test)]
mod bench;
mod graph;
mod parser;
mod solver;
mod step;
mod tour;
mod weight;

use self::graph::{Graph, Node};
use self::solver::SolverCx;

const ABOUT: &str = "
Parallel traveling salesman problem solver. Data input is expected to
be in TSPLIB format.

Suggested command:
    cargo run --release -- tsp bench data/tsp/dj15.tsp --seq-threshold 8
";

#[derive(Subcommand)]
enum Commands {
    /// Run the benchmark and print the timings
    Bench {
        #[clap(value_parser)]
        datafile: PathBuf,
    },
}

#[derive(Parser)]
#[clap(about = ABOUT)]
struct Args {
    #[clap(subcommand)]
    command: Commands,

    /// Node index from which to start the search
    #[clap(long, default_value_t = 0)]
    from: usize,

    /// Adjust sequential fallback threshold. Fall back to seq search when there
    /// are N or fewer nodes remaining. Lower values of N mean more parallelism.
    #[clap(long, default_value_t = 10)]
    seq_threshold: usize,
}

pub fn main(args: &[String]) {
    let args: Args = Parser::parse_from(args);

    match args.command {
        Commands::Bench { datafile } => {
            let _ = run_solver(&datafile, args.seq_threshold, args.from);
        }
    }
}

fn run_solver(datafile: &Path, seq_threshold: usize, from: usize) -> Result<(), ()> {
    let graph = match parse_solver(datafile) {
        Ok(g) => g,
        Err(e) => {
            println!("failed to parse `{}`: {}", datafile.display(), e);
            return Err(());
        }
    };

    println!("Graph size   : {} nodes.", graph.num_nodes());
    println!("Seq threshold: {} nodes.", seq_threshold);

    if from >= graph.num_nodes() {
        println!("Invalid node index given for `--from`: {}", from);
        return Err(());
    }

    let mut solver = SolverCx::new(&graph, seq_threshold);
    let par_start = Instant::now();
    solver.search_from(Node::new(from));
    let par_time = par_start.elapsed();

    let (path, weight) = solver.into_result();

    println!("Total search time: {:?}", par_time);
    if let Some(path) = path {
        println!("Cheapest path cost: {}", weight.to_usize());
        let mut output = "Cheapest path:".to_string();
        for node in path {
            output.push_str(&format!(" {}", node.index()));
        }
        println!("{}", output);
    } else {
        println!("No path found.");
    }

    Ok(())
}

fn parse_solver(datafile: &Path) -> Result<Graph, Box<dyn Error>> {
    let mut file = File::open(datafile)?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    let graph = parser::parse_tsp_data(&text)?;
    Ok(graph)
}
