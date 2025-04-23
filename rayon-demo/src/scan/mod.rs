use rayon::iter::*;

mod scan;
use self::scan::{scan_matmul, Procs};

#[test]
fn test_scan_matmul() {
    assert_eq!(
        scan_matmul(Procs::Sequential, 10),
        scan_matmul(Procs::Parallel, 10)
    );
}

#[test]
fn test_scan_addition() {
    let init = 0u64;
    let op = |state: &mut u64, x: &u64| {
        *state += x;
        Some(*state)
    };
    let op_par = |state: &u64, x: &u64| *state + x;

    for len in 0..100 {
        let v = vec![1u64; len];
        let scan_seq = v.iter().scan(init, op).collect::<Vec<u64>>();
        let scan_par = v.into_par_iter().scan(op_par, init).collect::<Vec<u64>>();
        assert_eq!(scan_seq, scan_par);
    }
}

#[cfg(test)]
mod bench;
