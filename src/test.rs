#![cfg(test)]

use super::execute;

#[test]
fn use_it() {
    let mut left: isize = 0;
    let mut right: isize = 0;
    execute(&mut [
        &mut || left = 22,
        &mut || right = 44
    ]);
    assert_eq!(left, 22);
    assert_eq!(right, 44);
}

#[cfg(test)]
fn fib(n: usize, depth: usize) -> usize {
    if n < 2 {
        n
    } else {
        let mut r1: usize = 0;
        let mut r2: usize = 0;
        if depth > 0 {
            execute(&mut [
                &mut || r1 = fib(n-1, depth-1),
                &mut || r2 = fib(n-2, depth-1),
            ]);
        } else {
            r1 = fib(n-1, 0);
            r2 = fib(n-2, 0);
        }
        r1 + r2
    }
}

#[test]
fn calc_fib() {
    let n: usize = 20;
    let res = fib(n, 3);
    assert_eq!(res, 6765);
}

#[cfg(test)]
fn quicksort(v: &mut [isize]) {
    if v.len() <= 1 {
        return;
    }

    let pivot_value = v[0]; // simplest possible thing...
    let mid = partition(pivot_value, v);
    let (left, right) = v.split_at_mut(mid);
    execute(&mut [
        &mut || quicksort(left),
        &mut || quicksort(right)
    ]);

    fn partition(pivot_value: isize,
                 v: &mut [isize])
                 -> usize
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
