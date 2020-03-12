const USAGE: &str = "
Usage: life bench [--size N] [--gens N]
       life play [--size N] [--gens N] [--fps N]
       life --help
Conway's Game of Life.

Commands:
    bench           Run the benchmark in different modes and print the timings.
    play            Run with a max frame rate and monitor CPU resources.
Options:
    --size N        Size of the game board (N x N) [default: 200]
    --gens N        Simulate N generations [default: 100]
    --fps N         Maximum frame rate [default: 60]
    -h, --help      Show this message.
";

use crate::cpu_time::{self, CpuMeasure};
use rand::distributions::Standard;
use rand::{thread_rng, Rng};
use std::iter::repeat;
use std::num::Wrapping;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use docopt::Docopt;
use rayon::iter::ParallelBridge;
use rayon::prelude::*;

#[cfg(test)]
mod bench;

#[derive(serde::Deserialize)]
pub struct Args {
    cmd_bench: bool,
    cmd_play: bool,
    flag_size: usize,
    flag_gens: usize,
    flag_fps: usize,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Board {
    board: Vec<bool>,
    survive: Arc<Vec<usize>>,
    born: Arc<Vec<usize>>,
    rows: usize,
    cols: usize,
}

impl Board {
    pub fn new(rows: usize, cols: usize) -> Board {
        let born = vec![3];
        let survive = vec![2, 3];

        Board::new_with_custom_rules(rows, cols, born, survive)
    }

    fn new_with_custom_rules(
        rows: usize,
        cols: usize,
        born: Vec<usize>,
        survive: Vec<usize>,
    ) -> Board {
        let new_board = repeat(false).take(rows * cols).collect();

        Board {
            board: new_board,
            born: Arc::new(born),
            survive: Arc::new(survive),
            rows,
            cols,
        }
    }

    fn len(&self) -> usize {
        self.rows * self.cols
    }

    fn next_board(&self, new_board: Vec<bool>) -> Board {
        assert!(new_board.len() == self.len());

        Board {
            board: new_board,
            born: self.born.clone(),
            survive: self.survive.clone(),
            rows: self.rows,
            cols: self.cols,
        }
    }

    pub fn random(&self) -> Board {
        let new_brd = thread_rng()
            .sample_iter(&Standard)
            .take(self.len())
            .collect();

        self.next_board(new_brd)
    }

    pub fn next_generation(&self) -> Board {
        let new_brd = (0..self.len())
            .map(|cell| self.successor_cell(cell))
            .collect();

        self.next_board(new_brd)
    }

    pub fn parallel_next_generation(&self) -> Board {
        let new_brd = (0..self.len())
            .into_par_iter()
            .map(|cell| self.successor_cell(cell))
            .collect();

        self.next_board(new_brd)
    }

    pub fn par_bridge_next_generation(&self) -> Board {
        let new_brd = (0..self.len())
            .par_bridge()
            .map(|cell| self.successor_cell(cell))
            .collect();

        self.next_board(new_brd)
    }

    fn cell_live(&self, x: usize, y: usize) -> bool {
        !(x >= self.cols || y >= self.rows) && self.board[y * self.cols + x]
    }

    fn living_neighbors(&self, x: usize, y: usize) -> usize {
        let Wrapping(x_1) = Wrapping(x) - Wrapping(1);
        let Wrapping(y_1) = Wrapping(y) - Wrapping(1);
        let neighbors = [
            self.cell_live(x_1, y_1),
            self.cell_live(x, y_1),
            self.cell_live(x + 1, y_1),
            self.cell_live(x_1, y + 0),
            self.cell_live(x + 1, y + 0),
            self.cell_live(x_1, y + 1),
            self.cell_live(x, y + 1),
            self.cell_live(x + 1, y + 1),
        ];
        neighbors.iter().filter(|&x| *x).count()
    }

    fn successor_cell(&self, cell: usize) -> bool {
        self.successor(cell % self.cols, cell / self.cols)
    }

    fn successor(&self, x: usize, y: usize) -> bool {
        let neighbors = self.living_neighbors(x, y);
        if self.cell_live(x, y) {
            self.survive.contains(&neighbors)
        } else {
            self.born.contains(&neighbors)
        }
    }
}

#[test]
fn test_life() {
    let mut brd1 = Board::new(200, 200).random();
    let mut brd2 = brd1.clone();

    for _ in 0..100 {
        brd1 = brd1.next_generation();
        brd2 = brd2.parallel_next_generation();

        assert_eq!(brd1, brd2);
    }
}

fn generations(board: Board, gens: usize) {
    let mut brd = board;
    for _ in 0..gens {
        brd = brd.next_generation();
    }
}

fn parallel_generations(board: Board, gens: usize) {
    let mut brd = board;
    for _ in 0..gens {
        brd = brd.parallel_next_generation();
    }
}

fn par_bridge_generations(board: Board, gens: usize) {
    let mut brd = board;
    for _ in 0..gens {
        brd = brd.par_bridge_next_generation();
    }
}

fn delay(last_start: Instant, min_interval: Duration) -> Instant {
    let mut current_time = Instant::now();
    let elapsed = current_time - last_start;
    if elapsed < min_interval {
        let delay = min_interval - elapsed;
        thread::sleep(delay);
        current_time += delay;
    }
    current_time
}

fn generations_limited(board: Board, gens: usize, min_interval: Duration) {
    let mut brd = board;
    let mut time = Instant::now();
    for _ in 0..gens {
        brd = brd.next_generation();
        time = delay(time, min_interval);
    }
}

fn parallel_generations_limited(board: Board, gens: usize, min_interval: Duration) {
    let mut brd = board;
    let mut time = Instant::now();
    for _ in 0..gens {
        brd = brd.parallel_next_generation();
        time = delay(time, min_interval);
    }
}

fn par_bridge_generations_limited(board: Board, gens: usize, min_interval: Duration) {
    let mut brd = board;
    let mut time = Instant::now();
    for _ in 0..gens {
        brd = brd.par_bridge_next_generation();
        time = delay(time, min_interval);
    }
}

fn measure(f: fn(Board, usize) -> (), args: &Args) -> Duration {
    let (n, gens) = (args.flag_size, args.flag_gens);
    let brd = Board::new(n, n).random();
    let start = Instant::now();

    f(brd, gens);

    start.elapsed()
}

struct CpuResult {
    actual_fps: f64,
    cpu_usage_percent: Option<f64>,
}

fn measure_cpu(f: fn(Board, usize, Duration) -> (), args: &Args) -> CpuResult {
    let (n, gens, rate) = (args.flag_size, args.flag_gens, args.flag_fps);
    let interval = Duration::from_secs_f64(1.0 / rate as f64);
    let brd = Board::new(n, n).random();

    let CpuMeasure {
        time_duration,
        cpu_usage_percent,
    } = cpu_time::measure_cpu(|| f(brd, gens, interval));

    CpuResult {
        actual_fps: gens as f64 / time_duration.as_secs_f64(),
        cpu_usage_percent,
    }
}

pub fn main(args: &[String]) {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(args).deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.cmd_bench {
        let serial = measure(generations, &args).as_nanos();
        println!("  serial: {:10} ns", serial);

        let parallel = measure(parallel_generations, &args).as_nanos();
        println!(
            "parallel: {:10} ns -> {:.2}x speedup",
            parallel,
            serial as f64 / parallel as f64
        );

        let par_bridge = measure(par_bridge_generations, &args).as_nanos();
        println!(
            "par_bridge: {:10} ns -> {:.2}x speedup",
            par_bridge,
            serial as f64 / par_bridge as f64
        );
    }

    if args.cmd_play {
        let serial = measure_cpu(generations_limited, &args);
        println!("  serial: {:.2} fps", serial.actual_fps);
        if let Some(cpu_usage) = serial.cpu_usage_percent {
            println!("    cpu usage: {:.1}%", cpu_usage);
        }

        let parallel = measure_cpu(parallel_generations_limited, &args);
        println!("parallel: {:.2} fps", parallel.actual_fps);
        if let Some(cpu_usage) = parallel.cpu_usage_percent {
            println!("  cpu usage: {:.1}%", cpu_usage);
        }

        let par_bridge = measure_cpu(par_bridge_generations_limited, &args);
        println!("par_bridge: {:.2} fps", par_bridge.actual_fps);
        if let Some(cpu_usage) = par_bridge.cpu_usage_percent {
            println!("  cpu usage: {:.1}%", cpu_usage);
        }
    }
}
