use ndarray::{Array, Dim};
use rayon::iter::*;
use std::time::{Duration, Instant};
use std::num::Wrapping;

const SIZE: usize = 10000;

enum Procs {
    Sequential,
    Parallel,
}

fn scan_sequential<T, P, I>(init: I, id: T, scan_op: P) -> Vec<T>
where
    T: Clone,
    I: Fn() -> T,
    P: FnMut(&mut T, &T) -> Option<T>,
{
    let v = vec![init(); SIZE];
    let scan = v.iter().scan(id, scan_op);
    scan.collect()
}

fn scan_parallel<T, P, I>(init: I, id: T, scan_op: P) -> Vec<T>
where
    T: Clone + Send + Sync,
    I: Fn() -> T,
    P: Fn(&T, &T) -> T + Sync,
{
    let v = vec![init(); SIZE];
    let scan = v.into_par_iter().with_min_len(SIZE / 100).scan(&scan_op, id);
    scan.collect()
}

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

#[bench]
fn scan_add_sequential(b: &mut test::Bencher) {
    b.iter(|| scan_add(Procs::Sequential));
}

#[bench]
fn scan_add_parallel(b: &mut test::Bencher) {
    b.iter(|| scan_add(Procs::Parallel));
}

#[test]
fn test_scan_add() {
    assert_eq!(scan_add(Procs::Sequential), scan_add(Procs::Parallel));
}

/******** Matrix multiplication with wrapping arithmetic *******/

type Matrix = Array<Wrapping<i32>, Dim<[usize; 2]>>;
fn scan_matmul(procs: Procs) -> Vec<Matrix> {
    const MAT_SIZE: usize = 50;
    let init = || {
        Array::from_iter((0..((MAT_SIZE * MAT_SIZE) as i32)).map(|x| Wrapping(x)))
            .into_shape((MAT_SIZE, MAT_SIZE))
            .unwrap()
    };
    let id = Array::eye(MAT_SIZE);

    match procs {
        Procs::Sequential => {
            let f = |state: &mut Matrix, x: &Matrix| {
                *state = state.dot(x);
                Some(state.clone())
            };

            scan_sequential(init, id, f)
        }
        Procs::Parallel => scan_parallel(init, id, |x, y| x.dot(y)),
    }
}

#[bench]
fn scan_matmul_sequential(b: &mut test::Bencher) {
    b.iter(|| scan_matmul(Procs::Sequential));
}

#[bench]
fn scan_matmul_parallel(b: &mut test::Bencher) {
    b.iter(|| scan_matmul(Procs::Parallel));
}

#[test]
fn test_scan_matmul() {
    assert_eq!(scan_matmul(Procs::Sequential), scan_matmul(Procs::Parallel));
}
