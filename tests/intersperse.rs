use rayon::prelude::*;

#[test]
fn check_intersperse() {
    let v: Vec<_> = (0..1000).into_par_iter().intersperse(-1).collect();
    assert_eq!(v.len(), 1999);
    for (i, x) in v.into_iter().enumerate() {
        assert_eq!(x, if i % 2 == 0 { i as i32 / 2 } else { -1 });
    }
}

#[test]
fn check_intersperse_again() {
    let v: Vec<_> = (0..1000)
        .into_par_iter()
        .intersperse(-1)
        .intersperse(-2)
        .collect();
    assert_eq!(v.len(), 3997);
    for (i, x) in v.into_iter().enumerate() {
        let y = match i % 4 {
            0 => i as i32 / 4,
            2 => -1,
            _ => -2,
        };
        assert_eq!(x, y);
    }
}

#[test]
fn check_intersperse_unindexed() {
    let v: Vec<_> = (0..1000).map(|i| i.to_string()).collect();
    let s = v.join(",");
    let s2 = v.join(";");
    let par: String = s.par_split(',').intersperse(";").collect();
    assert_eq!(par, s2);
}

#[test]
fn check_intersperse_producer() {
    (0..1000)
        .into_par_iter()
        .intersperse(-1)
        .zip_eq(0..1999)
        .for_each(|(x, i)| {
            assert_eq!(x, if i % 2 == 0 { i / 2 } else { -1 });
        });
}

#[test]
fn check_intersperse_rev() {
    (0..1000)
        .into_par_iter()
        .intersperse(-1)
        .zip_eq(0..1999)
        .rev()
        .for_each(|(x, i)| {
            assert_eq!(x, if i % 2 == 0 { i / 2 } else { -1 });
        });
}

#[test]
fn check_intersperse_split_folds() {
    // Producers split at an even index end with an interspersed item, which
    // must not be lost when the leaves are folded instead of iterated.
    let v: Vec<_> = (0..5).into_par_iter().intersperse(-1).take(2).collect();
    assert_eq!(v, [0, -1]);

    let v: Vec<_> = (0..5).into_par_iter().intersperse(-1).skip(1).collect();
    assert_eq!(v, [-1, 1, -1, 2, -1, 3, -1, 4]);

    let v: Vec<_> = (0..1000)
        .into_par_iter()
        .intersperse(-1)
        .with_max_len(1)
        .collect();
    assert_eq!(v.len(), 1999);
    for (i, x) in v.into_iter().enumerate() {
        assert_eq!(x, if i % 2 == 0 { i as i32 / 2 } else { -1 });
    }

    let sum: i32 = (0..1000)
        .into_par_iter()
        .intersperse(1000)
        .with_max_len(1)
        .sum();
    assert_eq!(sum, (0..1000).sum::<i32>() + 999 * 1000);
}
