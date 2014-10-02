#![cfg(test)]

use super::{execute, Section};

#[test]
fn use_it() {
    let mut left: int = 0;
    let mut right: int = 0;
    execute(&mut [
        || left = 22,
        || right = 44
    ]);
    assert_eq!(left, 22);
    assert_eq!(right, 44);
}

// #[test]
// fn use_it_bad() {
//     let mut left: int = 0;
//     let mut right: int = 0;
//     execute(&mut [
//         || left = 22,
//         || left = 44  //~ ERROR cannot borrow `left` as mutable more than once
//     ]);
//     assert_eq!(left, 22);
//     assert_eq!(right, 44);
// }
