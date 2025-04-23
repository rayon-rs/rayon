use ndarray::{Array, Dim};
use rayon::iter::*;
use std::num::Wrapping;

const SIZE: usize = 10000;

pub enum Procs {
    Sequential,
    Parallel,
}

pub fn scan_sequential<T, P, I>(init: I, id: T, scan_op: P) -> Vec<T>
where
    T: Clone,
    I: Fn() -> T,
    P: FnMut(&mut T, &T) -> Option<T>,
{
    let v = vec![init(); SIZE];
    let scan = v.iter().scan(id, scan_op);
    scan.collect()
}

pub fn scan_parallel<T, P, I>(init: I, id: T, scan_op: P) -> Vec<T>
where
    T: Clone + Send + Sync,
    I: Fn() -> T,
    P: Fn(&T, &T) -> T + Sync,
{
    let v = vec![init(); SIZE];
    let scan = v
        .into_par_iter()
        .with_min_len(SIZE / 100)
        .scan(&scan_op, id);
    scan.collect()
}

/******** Matrix multiplication with wrapping arithmetic *******/

type Matrix = Array<Wrapping<i32>, Dim<[usize; 2]>>;
pub fn scan_matmul(procs: Procs, mat_size: usize) -> Vec<Matrix> {
    let init = || {
        Array::from_iter((0..((mat_size * mat_size) as i32)).map(|x| Wrapping(x)))
            .into_shape((mat_size, mat_size))
            .unwrap()
    };
    let id = Array::eye(mat_size);

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
