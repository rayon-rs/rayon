#![cfg(test)]

use super::{execute, Section, TaskBody};

#[test]
fn use_execute() {
    let mut left: int = 0;
    let mut right: int = 0;
    execute(&mut [
        || left = 22,
        || right = 44
    ]);
    assert_eq!(left, 22);
    assert_eq!(right, 44);
}

#[test]
fn use_section() {
    let mut left: int = 0;
    let mut right: int = 0;

    {
        let mut section = Section::new();
        let body: &mut TaskBody = &mut || left += 1;
        section.fork(body);
        let body: &mut TaskBody = &mut || right += 1;
        section.fork(body);
    }

    assert_eq!(left, 1);
    assert_eq!(right, 1);
}

#[cfg(test)]
fn quicksort(v: &mut [int]) {
    if v.len() <= 1 {
        return;
    }

    let pivot_value = v[0]; // simplest possible thing...
    let mid = partition(pivot_value, v);
    let (left, right) = v.split_at_mut(mid);
    execute(&mut [
        || quicksort(left),
        || quicksort(right)
    ]);

    fn partition(pivot_value: int,
                 v: &mut [int])
                 -> uint
    {
        // Invariant:
        //     .. l ==> less than or equal to pivot
        //     r .. ==> greater than pivot
        let mut l = 0;
        let mut r = v.len() - 1;
        while l <= r {
            if v[l] > pivot_value {
                v.swap(l, r);
                r -= 1;
            } else if v[r] <= pivot_value {
                v.swap(l, r);
                l += 1;
            } else {
                l += 1;
                r -= 1;
            }
        }
        return l;
    }
}

#[test]
fn call_quicksort() {
    let mut v = [55, 12, 86, 8, 3, 5];
    quicksort(v.as_mut_slice());
    let mut bound = 0;
    for &elem in v.iter() {
        assert!(elem >= bound);
        bound = elem;
    }
}

//#[test]
//fn use_it_bad() {
//    let mut left: int = 0;
//    let mut right: int = 0;
//    execute(&mut [
//        || left = 22,
//        || left = 44  //~ ERROR cannot borrow `left` as mutable more than once
//    ]);
//    assert_eq!(left, 22);
//    assert_eq!(right, 44);
//}
//
//#[test]
//fn illegal_section() {
//    let mut left: int = 0;
//    let mut right: int = 0;
//
//    {
//        let mut section = Section::new();
//        let body: &mut TaskBody = &mut || left += 1;
//        section.fork(body);
//        let body: &mut TaskBody = &mut || left += 1; //~ ERROR cannot borrow `left`
//        section.fork(body);
//    }
//
//    assert_eq!(left, 1);
//    assert_eq!(right, 1);
//}
//
//#[test]
//fn illegal_section() {
//    let mut left = 0;
//    let mut section = Section::new();
//
//    let body: &mut TaskBody = &mut || left += 1;
//    section.fork(body);
//
//    {
//        let mut right = 0;
//        let body: &mut TaskBody = &mut || right += 1; //~ ERROR cannot infer
//        section.fork(body);
//    }
//}
//
//fn illegal_section<'a>() -> Section<'a> {
//    let mut left = 0;
//    let mut section = Section::new();
//
//    let body: &mut TaskBody = &mut || left += 1; //~ ERROR cannot infer
//    section.fork(body);
//
//    section
//}
//
//fn illegal_section() {
//    let mut left = 0u;
//    let mut section = Section::new();
//
//    let body: &mut TaskBody = &mut || left += 1;
//    section.fork(body);
//
//    left += 1; //~ ERROR cannot assign
//}
//
//fn illegal_section() {
//    let mut left = 0u;
//    let mut right = 0u;
//    let mut section = Section::new();
//
//    let body: &mut TaskBody = &mut || left += 1;
//    section.fork(body);
//
//    right = left; //~ ERROR cannot use `left`
//}
