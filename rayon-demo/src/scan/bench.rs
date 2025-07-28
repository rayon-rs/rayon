use std::time::{Duration, Instant};

use super::scan::{scan_matmul, scan_parallel, scan_sequential, Procs};

/******* Addition with artificial delay *******/

const DELAY: Duration = Duration::from_nanos(10);
fn wait() -> i32 {
    let time = Instant::now();

    let mut sum = 0;
    while time.elapsed() < DELAY {
        sum += 1;
    }
    sum
}

fn scan_add(procs: Procs) -> Vec<i32> {
    let init = || 2;
    let id = 0;

    match procs {
        Procs::Sequential => {
            let f = |state: &mut i32, x: &i32| {
                test::black_box(wait());
                *state += x;
                Some(*state)
            };
            scan_sequential(init, id, f)
        }
        Procs::Parallel => {
            let f = |x: &i32, y: &i32| {
                test::black_box(wait());
                *x + *y
            };
            scan_parallel(init, id, f)
        }
    }
}

#[test]
fn test_scan_add() {
    assert_eq!(scan_add(Procs::Sequential), scan_add(Procs::Parallel));
}

#[bench]
fn scan_add_sequential(b: &mut test::Bencher) {
    b.iter(|| scan_add(Procs::Sequential));
}

#[bench]
fn scan_add_parallel(b: &mut test::Bencher) {
    b.iter(|| scan_add(Procs::Parallel));
}

#[bench]
fn scan_matmul_sequential(b: &mut test::Bencher) {
    b.iter(|| scan_matmul(Procs::Sequential, 50));
}

#[bench]
fn scan_matmul_parallel(b: &mut test::Bencher) {
    b.iter(|| scan_matmul(Procs::Parallel, 50));
}
