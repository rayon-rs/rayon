use super::*;

#[test]
pub fn execute() {
    let a: Vec<i32> = (0..1024).collect();
    let mut b = vec![];
    a.into_par_iter()
     .map(|&i| i + 1)
     .collect_into(&mut b);
    let c: Vec<i32> = (0..1024).map(|i| i+1).collect();
    assert_eq!(b, c);
}

#[test]
pub fn map_reduce() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.into_par_iter()
              .map(|&i| i + 1)
              .sum();
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1, r2);
}

#[test]
pub fn map_reduce_with() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.into_par_iter()
              .map(|&i| i + 1)
              .reduce_with(|i, j| i + j);
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1.unwrap(), r2);
}

#[test]
pub fn map_reduce_weighted() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.into_par_iter()
              .map(|&i| i + 1)
              .weight(2.0)
              .reduce_with(|i, j| i + j);
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1.unwrap(), r2);
}

#[test]
pub fn check_weight() {
    let a: Vec<i32> = (0..1024).collect();

    let len1 = {
        let (shared, mut state) = a.into_par_iter().state();
        state.len(&shared)
    };

    let len2 = {
        let (shared, mut state) = a.into_par_iter()
                               .weight(2.0)
                               .state();
        state.len(&shared)
    };

    assert_eq!(len1.cost * 2.0, len2.cost);
}

#[test]
pub fn check_enumerate() {
    let a: Vec<usize> = (0..1024).rev().collect();

    let mut b = vec![];
    a.into_par_iter()
     .enumerate()
     .map(|(i, &x)| i + x)
     .collect_into(&mut b);
    assert!(b.iter().all(|&x| x == a.len() - 1));
}
