use rayon::prelude::*;
use std::collections::HashSet;

#[test]
fn cartesian_ranges() {
    let a: Vec<(usize, usize)> = (0..3).into_par_iter().cartesian_product(0..3).collect();
    let b: HashSet<(usize, usize)> = HashSet::from_iter(a.into_iter());
    assert!(b.contains(&(0, 0)));
    assert!(b.contains(&(0, 1)));
    assert!(b.contains(&(0, 2)));
    assert!(b.contains(&(1, 0)));
    assert!(b.contains(&(1, 1)));
    assert!(b.contains(&(1, 2)));
    assert!(b.contains(&(2, 0)));
    assert!(b.contains(&(2, 1)));
    assert!(b.contains(&(2, 2)));
}

#[test]
fn cartesian_vecs() {
    let a: Vec<(f64, f64)> = vec![0.1, 1.2, 2.4]
        .into_par_iter()
        .cartesian_product(vec![4.8, 16.32, 32.64])
        .collect();
    assert!(a.contains(&(0.1, 4.8)));
    assert!(a.contains(&(0.1, 16.32)));
    assert!(a.contains(&(0.1, 32.64)));
    assert!(a.contains(&(1.2, 4.8)));
    assert!(a.contains(&(1.2, 16.32)));
    assert!(a.contains(&(1.2, 32.64)));
    assert!(a.contains(&(2.4, 4.8)));
    assert!(a.contains(&(2.4, 16.32)));
    assert!(a.contains(&(2.4, 32.64)));
}

#[test]
fn cartesian_chain() {
    let a: Vec<(usize, usize, usize)> = (0..2)
        .into_par_iter()
        .cartesian_product(0..2)
        .cartesian_product(0..2)
        .map(|((a, b), c)| (a, b, c))
        .collect();
    let b: HashSet<(usize, usize, usize)> = HashSet::from_iter(a.into_iter());
    assert!(b.contains(&(0, 0, 0)));
    assert!(b.contains(&(0, 0, 1)));
    assert!(b.contains(&(0, 1, 0)));
    assert!(b.contains(&(0, 1, 1)));
    assert!(b.contains(&(1, 0, 0)));
    assert!(b.contains(&(1, 0, 1)));
    assert!(b.contains(&(1, 1, 0)));
    assert!(b.contains(&(1, 1, 1)));
}
