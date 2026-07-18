use rayon::prelude::*;

/// The reference (sequential) cartesian product, row-major.
fn seq(m: usize, n: usize) -> Vec<(usize, usize)> {
    (0..m).flat_map(|a| (0..n).map(move |b| (a, b))).collect()
}

#[test]
fn order_matches_sequential() {
    for m in 0..7 {
        for n in 0..7 {
            let par: Vec<(usize, usize)> = (0..m).into_par_iter().cartesian_product(0..n).collect();
            assert_eq!(par, seq(m, n), "m={m} n={n}");
        }
    }
}

#[test]
fn is_indexed() {
    // known length
    assert_eq!((0..3).into_par_iter().cartesian_product(0..4).len(), 12);
    assert_eq!((0..3).into_par_iter().cartesian_product(0..4).count(), 12);

    // collect_into_vec (preallocation path -- the whole point of issue #754)
    let mut v = Vec::new();
    (0..3)
        .into_par_iter()
        .cartesian_product(0..4)
        .collect_into_vec(&mut v);
    assert_eq!(v, seq(3, 4));

    // composes with other indexed adaptors
    let enumerated: Vec<(usize, (usize, usize))> = (0..2)
        .into_par_iter()
        .cartesian_product(0..2)
        .enumerate()
        .collect();
    assert_eq!(
        enumerated,
        vec![(0, (0, 0)), (1, (0, 1)), (2, (1, 0)), (3, (1, 1))]
    );

    let reversed: Vec<(usize, usize)> = (0..2)
        .into_par_iter()
        .cartesian_product(0..2)
        .rev()
        .collect();
    assert_eq!(reversed, vec![(1, 1), (1, 0), (0, 1), (0, 0)]);

    let skipped: Vec<(usize, usize)> = (0..3)
        .into_par_iter()
        .cartesian_product(0..2)
        .skip(2)
        .take(2)
        .collect();
    assert_eq!(skipped, vec![(1, 0), (1, 1)]);
}

#[test]
fn empty_inputs() {
    let a: Vec<(usize, usize)> = (0..0).into_par_iter().cartesian_product(0..5).collect();
    assert!(a.is_empty());
    let b: Vec<(usize, usize)> = (0..5).into_par_iter().cartesian_product(0..0).collect();
    assert!(b.is_empty());
    let c: Vec<(usize, usize)> = (0..0).into_par_iter().cartesian_product(0..0).collect();
    assert!(c.is_empty());
}

#[test]
fn owned_clone_items() {
    // non-Copy, Clone items on both axes
    let rows = vec!["a".to_string(), "b".to_string()];
    let cols = vec!["x".to_string(), "y".to_string()];
    let got: Vec<(String, String)> = rows.par_iter().cloned().cartesian_product(cols).collect();
    assert_eq!(
        got,
        vec![
            ("a".into(), "x".into()),
            ("a".into(), "y".into()),
            ("b".into(), "x".into()),
            ("b".into(), "y".into()),
        ]
    );
}

#[test]
fn large_parallel() {
    // usize ranges are indexed (u64 ranges are not; same limitation as `zip`).
    let (m, n) = (500usize, 300usize);
    let sum: u64 = (0..m)
        .into_par_iter()
        .cartesian_product(0..n)
        .map(|(a, b)| (a * b) as u64)
        .sum();
    let expected: u64 = (0..m)
        .flat_map(|a| (0..n).map(move |b| (a * b) as u64))
        .sum();
    assert_eq!(sum, expected);
}
