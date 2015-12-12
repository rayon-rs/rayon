use super::*;

#[test]
pub fn execute() {
    let mut v: Vec<i32> = (0..1024).collect();
    let mut w = vec![];
    let w = v.into_par_iter()
             .map(&|&i| i + 1)
             .collect_into(&mut w);
}
