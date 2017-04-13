use std::path::Path;
use test;

use super::parse_solver;
use super::graph::Node;
use super::solver::SolverCx;

fn run_dir(
    b: &mut test::Bencher,
    name: &str,
    seq_threshold: usize,
    exp_weight: usize,
    exp_path: Vec<usize>,
) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tsp_path = manifest_dir.join("data/tsp/").join(name);
    let graph = parse_solver(&tsp_path).unwrap();
    let mut solution = None;
    b.iter(
        || {
            let mut solver = SolverCx::new(&graph, seq_threshold);
            solver.search_from(Node::new(0));
            solution = Some(solver.into_result());
        },
    );
    let (path, weight) = solution.unwrap();
    let mut path: Vec<usize> = path.unwrap().iter().map(|n| n.index()).collect();
    if path.iter().rev().lt(&path) {
        path.reverse(); // normalize the direction
    }
    assert_eq!(
        exp_weight,
        weight.to_usize(),
        "actual weight ({:?}) did not match expectation ({:?})",
        weight,
        exp_weight
    );
    assert_eq!(
        exp_path,
        path,
        "best path ({:?}) did not match expectation ({:?})",
        path,
        exp_path
    );
}

#[bench]
fn dj10(b: &mut test::Bencher) {
    // these numbers are tuned to "not take THAT long" in cargo bench,
    // basically, but still exercise the spawning stuff -- each run
    // should spawn 6! (720) tasks or so this way.
    run_dir(
        b,
        "dj10.tsp",
        4,
        2577,
        vec![0, 1, 3, 2, 4, 6, 8, 7, 5, 9, 0],
    );
}
