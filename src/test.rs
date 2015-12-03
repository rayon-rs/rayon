#![cfg(test)]

use api::*;
use rand::{Rng, SeedableRng, XorShiftRng};

fn quick_sort<T:PartialOrd+Send>(v: &mut [T]) {
    if v.len() <= 1 {
        return;
    }

    let mid = partition(v);
    let (lo, hi) = v.split_at_mut(mid);
    join(|| quick_sort(lo),
         || quick_sort(hi));
}

fn partition<T:PartialOrd+Send>(v: &mut [T]) -> usize {
    let pivot = v.len() - 1;
    let mut i = 0;
    for j in 0..pivot {
        if v[j] <= v[pivot] {
            v.swap(i, j);
            i += 1;
        }
    }
    v.swap(i, pivot);
    i
}

#[test]
fn sort() {
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    let mut data: Vec<_> = (0..2048).map(|_| rng.next_u32()).collect();

    quick_sort(&mut data);

    let mut sorted_data = data.clone();
    sorted_data.sort();

    assert_eq!(data, sorted_data);
}
