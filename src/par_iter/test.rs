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
