const USAGE: &'static str = "
Usage: life bench [--size N] [--gens N]
       life --help
Conway's Game of Life.

Commands:
    bench           Run the benchmark in different modes and print the timings.
Options:
    --size N        Size of the game board (N x N) [default: 200]
    --gens N        Simulate N generations [default: 100]
    -h, --help      Show this message.
";


use rand::{thread_rng, Rng};
use std::iter::repeat;
use std::num::Wrapping;
use std::sync::Arc;
use time;

use docopt::Docopt;
use rayon::prelude::*;
use rayon::iter::AsParallel;

#[cfg(test)]
mod bench;

#[derive(Deserialize)]
pub struct Args {
    cmd_bench: bool,
    flag_size: usize,
    flag_gens: usize,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Board {
    board: Vec<bool>,
    survive: Arc<Vec<usize>>,
    born: Arc<Vec<usize>>,
    rows: usize,
    cols: usize
}

impl Board {
    pub fn new(rows: usize, cols: usize) -> Board {
        let born = vec![3];
        let survive = vec![2, 3];

        Board::new_with_custom_rules(rows, cols, born, survive)
    }

    fn new_with_custom_rules(rows: usize, cols: usize, born: Vec<usize>, survive: Vec<usize>) -> Board {
        let new_board = repeat(false).take(rows * cols).collect();

        Board { board  : new_board,
                born   : Arc::new(born),
                survive: Arc::new(survive),
                rows   : rows,
                cols   : cols }
    }

    fn len(&self) -> usize {
        self.rows * self.cols
    }

    fn next_board(&self, new_board: Vec<bool>) -> Board {
        assert!(new_board.len() == self.len());

        Board { board  : new_board,
                born   : self.born.clone(),
                survive: self.survive.clone(),
                rows   : self.rows,
                cols   : self.cols }
    }

    pub fn random(&self) -> Board {
        let new_brd = thread_rng().gen_iter().take(self.len()).collect();

        self.next_board(new_brd)
    }

    pub fn next_generation(&self) -> Board {
        let new_brd = (0..self.len()).map(|cell| self.successor_cell(cell)).collect();

        self.next_board(new_brd)
    }

    pub fn parallel_next_generation(&self) -> Board {
        let new_brd = (0..self.len())
            .into_par_iter()
            .map(|cell| self.successor_cell(cell))
            .collect();

        self.next_board(new_brd)
    }

    pub fn as_parallel_next_generation(&self) -> Board {
        let new_brd = (0..self.len())
            .as_parallel()
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
            self.cell_live(x_1, y_1), self.cell_live(x, y_1), self.cell_live(x+1, y_1),
            self.cell_live(x_1, y+0),                         self.cell_live(x+1, y+0),
            self.cell_live(x_1, y+1), self.cell_live(x, y+1), self.cell_live(x+1, y+1),
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
    for _ in 0..gens { brd = brd.next_generation(); }
}

fn parallel_generations(board: Board, gens: usize) {
    let mut brd = board;
    for _ in 0..gens { brd = brd.parallel_next_generation(); }
}

fn as_parallel_generations(board: Board, gens: usize) {
    let mut brd = board;
    for _ in 0..gens { brd = brd.as_parallel_next_generation(); }
}

fn measure(f: fn(Board, usize) -> (), args: &Args) -> u64 {
    let (n, gens) = (args.flag_size, args.flag_gens);
    let brd = Board::new(n, n).random();
    let start = time::precise_time_ns();

    f(brd, gens);

    time::precise_time_ns() - start
}

pub fn main(args: &[String]) {
    let args: Args =
        Docopt::new(USAGE)
            .and_then(|d| d.argv(args).deserialize())
            .unwrap_or_else(|e| e.exit());

    if args.cmd_bench {
        let serial = measure(generations, &args);
        println!("  serial: {:10} ns", serial);

        let parallel = measure(parallel_generations, &args);
        println!("parallel: {:10} ns -> {:.2}x speedup", parallel,
                 serial as f64 / parallel as f64);

        let as_parallel = measure(as_parallel_generations, &args);
        println!("as_parallel: {:10} ns -> {:.2}x speedup", as_parallel,
                 serial as f64 / as_parallel as f64);
    }
}
