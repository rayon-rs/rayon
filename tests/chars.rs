use rayon::prelude::*;
use std::char;
use std::collections::HashSet;

#[test]
fn half_open_correctness() {
    let low = char::from_u32(0xD800 - 10).unwrap();
    let high = char::from_u32(0xE000 + 10).unwrap();

    let range = low..high;

    let std_iter: HashSet<char> = range.clone().collect();
    let par_iter: HashSet<char> = range.into_par_iter().collect();

    assert_eq!(std_iter, par_iter);
}

#[test]
fn closed_correctness() {
    let low = char::from_u32(0xD800 - 10).unwrap();
    let high = char::from_u32(0xE000 + 10).unwrap();

    let range = low..=high;

    let std_iter: HashSet<char> = range.clone().collect();
    let par_iter: HashSet<char> = range.into_par_iter().collect();

    assert_eq!(std_iter, par_iter);
}
