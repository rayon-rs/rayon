use std::sync::atomic::{AtomicUsize, Ordering};

use super::internal::*;
use super::*;

use rand::{Rng, SeedableRng, XorShiftRng};
use std::collections::LinkedList;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::collections::{BinaryHeap, VecDeque};
use std::f64;

fn is_bounded<T: ExactParallelIterator>(_: T) {}
fn is_exact<T: ExactParallelIterator>(_: T) {}
fn is_indexed<T: IndexedParallelIterator>(_: T) {}

#[test]
pub fn execute() {
    let a: Vec<i32> = (0..1024).collect();
    let mut b = vec![];
    a.par_iter()
        .map(|&i| i + 1)
        .collect_into(&mut b);
    let c: Vec<i32> = (0..1024).map(|i| i + 1).collect();
    assert_eq!(b, c);
}

#[test]
pub fn execute_cloned() {
    let a: Vec<i32> = (0..1024).collect();
    let mut b: Vec<i32> = vec![];
    a.par_iter()
        .weight_max()
        .cloned()
        .collect_into(&mut b);
    let c: Vec<i32> = (0..1024).collect();
    assert_eq!(b, c);
}

#[test]
pub fn execute_range() {
    let a = 0i32..1024;
    let mut b = vec![];
    a.into_par_iter()
        .map(|i| i + 1)
        .collect_into(&mut b);
    let c: Vec<i32> = (0..1024).map(|i| i + 1).collect();
    assert_eq!(b, c);
}

#[test]
pub fn execute_unindexed_range() {
    let a = 0i64..1024;
    let b: LinkedList<i64> = a.into_par_iter().map(|i| i + 1).collect();
    let c: LinkedList<i64> = (0..1024).map(|i| i + 1).collect();
    assert_eq!(b, c);
}

#[test]
pub fn execute_strings() {
    let mut rng = XorShiftRng::from_seed([14159, 26535, 89793, 23846]);
    let s: String = rng.gen_iter::<char>().take(1024).collect();

    let par_chars: String = s.par_chars().collect();
    assert_eq!(s, par_chars);

    let par_even: String = s.par_chars().filter(|&c| (c as u32) & 1 == 0).collect();
    let ser_even: String = s.chars().filter(|&c| (c as u32) & 1 == 0).collect();
    assert_eq!(par_even, ser_even);
}

#[test]
pub fn check_map_exact_and_bounded() {
    let a = [1, 2, 3];
    is_bounded(a.par_iter().map(|x| x));
    is_exact(a.par_iter().map(|x| x));
    is_indexed(a.par_iter().map(|x| x));
}

#[test]
pub fn map_sum() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.par_iter()
        .weight_max()
        .map(|&i| i + 1)
        .sum();
    let r2 = a.iter()
        .map(|&i| i + 1)
        .fold(0, |a, b| a + b);
    assert_eq!(r1, r2);
}

#[test]
pub fn map_reduce() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.par_iter()
        .weight_max()
        .map(|&i| i + 1)
        .reduce(|| 0, |i, j| i + j);
    let r2 = a.iter()
        .map(|&i| i + 1)
        .fold(0, |a, b| a + b);
    assert_eq!(r1, r2);
}

#[test]
pub fn map_reduce_with() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.par_iter()
        .weight_max()
        .map(|&i| i + 1)
        .reduce_with(|i, j| i + j);
    let r2 = a.iter()
        .map(|&i| i + 1)
        .fold(0, |a, b| a + b);
    assert_eq!(r1, Some(r2));
}

#[test]
pub fn fold_map_reduce() {
    // Kind of a weird test, but it demonstrates various
    // transformations that are taking place. Relies on
    // `weight_max().fold()` being equivalent to `map()`.
    //
    // Take each number from 0 to 32 and fold them by appending to a
    // vector.  Because of `weight_max`, this will produce 32 vectors,
    // each with one item.  We then collect all of these into an
    // individual vector by mapping each into their own vector (so we
    // have Vec<Vec<i32>>) and then reducing those into a single
    // vector.
    let r1 = (0_i32..32)
        .into_par_iter()
        .weight_max()
        .fold(|| vec![], |mut v, e| {
            v.push(e);
            v
        })
        .map(|v| vec![v])
        .reduce_with(|mut v_a, v_b| {
            v_a.extend(v_b);
            v_a
        });
    assert_eq!(r1,
               Some(vec![vec![0], vec![1], vec![2], vec![3], vec![4], vec![5], vec![6], vec![7],
                         vec![8], vec![9], vec![10], vec![11], vec![12], vec![13], vec![14],
                         vec![15], vec![16], vec![17], vec![18], vec![19], vec![20], vec![21],
                         vec![22], vec![23], vec![24], vec![25], vec![26], vec![27], vec![28],
                         vec![29], vec![30], vec![31]]));
}

#[test]
pub fn check_weight_exact_and_bounded() {
    let a = [1, 2, 3];
    is_bounded(a.par_iter().weight(2.0));
    is_exact(a.par_iter().weight(2.0));
    is_indexed(a.par_iter().weight(2.0));
}

#[test]
pub fn check_enumerate() {
    let a: Vec<usize> = (0..1024).rev().collect();

    let mut b = vec![];
    a.par_iter()
        .enumerate()
        .map(|(i, &x)| i + x)
        .collect_into(&mut b);
    assert!(b.iter().all(|&x| x == a.len() - 1));
}

#[test]
pub fn check_indices_after_enumerate_split() {
    let a: Vec<i32> = (0..1024).collect();
    a.par_iter().enumerate().with_producer(WithProducer);

    struct WithProducer;
    impl<'a> ProducerCallback<(usize, &'a i32)> for WithProducer {
        type Output = ();
        fn callback<P>(self, producer: P)
            where P: Producer<Item = (usize, &'a i32)>
        {
            let (a, b) = producer.split_at(512);
            for ((index, value), trusted_index) in a.into_iter().zip(0..) {
                assert_eq!(index, trusted_index);
                assert_eq!(index, *value as usize);
            }
            for ((index, value), trusted_index) in b.into_iter().zip(512..) {
                assert_eq!(index, trusted_index);
                assert_eq!(index, *value as usize);
            }
        }
    }
}

#[test]
pub fn check_increment() {
    let mut a: Vec<usize> = (0..1024).rev().collect();

    a.par_iter_mut()
        .enumerate()
        .for_each(|(i, v)| *v += i);

    assert!(a.iter().all(|&x| x == a.len() - 1));
}

#[test]
pub fn check_skip() {
    let a: Vec<usize> = (0..1024).collect();

    let mut v1 = Vec::new();
    a.par_iter().skip(16).collect_into(&mut v1);
    let v2 = a.iter().skip(16).collect::<Vec<_>>();
    assert_eq!(v1, v2);

    let mut v1 = Vec::new();
    a.par_iter().skip(2048).collect_into(&mut v1);
    let v2 = a.iter().skip(2048).collect::<Vec<_>>();
    assert_eq!(v1, v2);

    let mut v1 = Vec::new();
    a.par_iter().skip(0).collect_into(&mut v1);
    let v2 = a.iter().skip(0).collect::<Vec<_>>();
    assert_eq!(v1, v2);

    // Check that the skipped elements side effects are executed
    use std::sync::atomic::{AtomicUsize, Ordering};
    let num = AtomicUsize::new(0);
    a.par_iter().map(|&n| num.fetch_add(n, Ordering::Relaxed)).skip(512).count();
    assert_eq!(num.load(Ordering::Relaxed), a.iter().sum());
}

#[test]
pub fn check_take() {
    let a: Vec<usize> = (0..1024).collect();

    let mut v1 = Vec::new();
    a.par_iter().take(16).collect_into(&mut v1);
    let v2 = a.iter().take(16).collect::<Vec<_>>();
    assert_eq!(v1, v2);

    let mut v1 = Vec::new();
    a.par_iter().take(2048).collect_into(&mut v1);
    let v2 = a.iter().take(2048).collect::<Vec<_>>();
    assert_eq!(v1, v2);

    let mut v1 = Vec::new();
    a.par_iter().take(0).collect_into(&mut v1);
    let v2 = a.iter().take(0).collect::<Vec<_>>();
    assert_eq!(v1, v2);
}

#[test]
pub fn check_inspect() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let a = AtomicUsize::new(0);
    let b = (0_usize..1024)
        .into_par_iter()
        .weight_max()
        .inspect(|&i| {
            a.fetch_add(i, Ordering::Relaxed);
        })
        .sum();

    assert_eq!(a.load(Ordering::Relaxed), b);
}

#[test]
pub fn check_move() {
    let a = vec![vec![1, 2, 3]];
    let ptr = a[0].as_ptr();

    let mut b = vec![];
    a.into_par_iter().collect_into(&mut b);

    // a simple move means the inner vec will be completely unchanged
    assert_eq!(ptr, b[0].as_ptr());
}

#[test]
pub fn check_drops() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let c = AtomicUsize::new(0);
    let a = vec![DropCounter(&c); 10];

    let mut b = vec![];
    a.clone().into_par_iter().collect_into(&mut b);
    assert_eq!(c.load(Ordering::Relaxed), 0);

    b.into_par_iter();
    assert_eq!(c.load(Ordering::Relaxed), 10);

    a.into_par_iter().with_producer(Partial);
    assert_eq!(c.load(Ordering::Relaxed), 20);


    #[derive(Clone)]
    struct DropCounter<'a>(&'a AtomicUsize);
    impl<'a> Drop for DropCounter<'a> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct Partial;
    impl<'a> ProducerCallback<DropCounter<'a>> for Partial {
        type Output = ();
        fn callback<P>(self, producer: P)
            where P: Producer<Item = DropCounter<'a>>
        {
            let (a, _) = producer.split_at(5);
            a.into_iter().next();
        }
    }
}

#[test]
pub fn check_slice_exact_and_bounded() {
    let a = vec![1, 2, 3];
    is_bounded(a.par_iter());
    is_exact(a.par_iter());
    is_indexed(a.par_iter());
}

#[test]
pub fn check_slice_mut_exact_and_bounded() {
    let mut a = vec![1, 2, 3];
    is_bounded(a.par_iter_mut());
    is_exact(a.par_iter_mut());
    is_indexed(a.par_iter_mut());
}

#[test]
pub fn check_vec_exact_and_bounded() {
    let a = vec![1, 2, 3];
    is_bounded(a.clone().into_par_iter());
    is_exact(a.clone().into_par_iter());
    is_indexed(a.clone().into_par_iter());
}

#[test]
pub fn check_range_exact_and_bounded() {
    is_bounded((1..5).into_par_iter());
    is_exact((1..5).into_par_iter());
    is_indexed((1..5).into_par_iter());
}

#[test]
pub fn check_cmp_direct() {
    let a = (0..1024).into_par_iter();
    let b = (0..1024).into_par_iter();

    let result = a.cmp(b);

    assert!(result == ::std::cmp::Ordering::Equal);
}

#[test]
pub fn check_cmp_to_seq() {
    assert_eq!((0..1024).into_par_iter().cmp(0..1024),
               (0..1024).cmp(0..1024));
}

#[test]
pub fn check_cmp_rng_to_seq() {
    use rand::{Rng, SeedableRng, XorShiftRng};

    let mut rng = XorShiftRng::from_seed([11507, 46649, 55961, 20171]);
    let a: Vec<i32> = rng.gen_iter().take(1024).collect();
    let b: Vec<i32> = rng.gen_iter().take(1024).collect();
    for i in 0..a.len() {
        let par_result = a[i..].par_iter().cmp(b[i..].par_iter());
        let seq_result = a[i..].iter().cmp(b[i..].iter());

        assert_eq!(par_result, seq_result);
    }
}

#[test]
pub fn check_cmp_lt_direct() {
    let a = (0..1024).into_par_iter();
    let b = (1..1024).into_par_iter();

    let result = a.cmp(b);

    assert!(result == ::std::cmp::Ordering::Less);
}

#[test]
pub fn check_cmp_lt_to_seq() {
    assert_eq!((0..1024).into_par_iter().cmp(1..1024),
               (0..1024).cmp(1..1024))
}

#[test]
pub fn check_cmp_gt_direct() {
    let a = (1..1024).into_par_iter();
    let b = (0..1024).into_par_iter();

    let result = a.cmp(b);

    assert!(result == ::std::cmp::Ordering::Greater);
}

#[test]
pub fn check_cmp_gt_to_seq() {
    assert_eq!((1..1024).into_par_iter().cmp(0..1024),
               (1..1024).cmp(0..1024))
}

#[test]
pub fn check_partial_cmp_direct() {
    let a = (0..1024).into_par_iter();
    let b = (0..1024).into_par_iter();

    let result = a.partial_cmp(b);

    assert!(result == Some(::std::cmp::Ordering::Equal));
}

#[test]
pub fn check_partial_cmp_to_seq() {
    let par_result = (0..1024).into_par_iter().partial_cmp(0..1024);
    let seq_result = (0..1024).partial_cmp(0..1024);
    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_partial_cmp_rng_to_seq() {
    use rand::{Rng, SeedableRng, XorShiftRng};

    let mut rng = XorShiftRng::from_seed([9346, 26355, 87943, 28346]);
    let a: Vec<i32> = rng.gen_iter().take(1024).collect();
    let b: Vec<i32> = rng.gen_iter().take(1024).collect();
    for i in 0..a.len() {
        let par_result = a[i..].par_iter().partial_cmp(b[i..].par_iter());
        let seq_result = a[i..].iter().partial_cmp(b[i..].iter());

        assert_eq!(par_result, seq_result);
    }
}

#[test]
pub fn check_partial_cmp_lt_direct() {
    let a = (0..1024).into_par_iter();
    let b = (1..1024).into_par_iter();

    let result = a.partial_cmp(b);

    assert!(result == Some(::std::cmp::Ordering::Less));
}

#[test]
pub fn check_partial_cmp_lt_to_seq() {
    let par_result = (0..1024).into_par_iter().partial_cmp(1..1024);
    let seq_result = (0..1024).partial_cmp(1..1024);
    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_partial_cmp_gt_direct() {
    let a = (1..1024).into_par_iter();
    let b = (0..1024).into_par_iter();

    let result = a.partial_cmp(b);

    assert!(result == Some(::std::cmp::Ordering::Greater));
}

#[test]
pub fn check_partial_cmp_gt_to_seq() {
    let par_result = (1..1024).into_par_iter().partial_cmp(0..1024);
    let seq_result = (1..1024).partial_cmp(0..1024);
    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_partial_cmp_none_direct() {
    let a = vec![f64::NAN, 0.0];
    let b = vec![0.0, 1.0];

    let result = a.par_iter().partial_cmp(b.par_iter());

    assert!(result == None);
}

#[test]
pub fn check_partial_cmp_none_to_seq() {
    let a = vec![f64::NAN, 0.0];
    let b = vec![0.0, 1.0];

    let par_result = a.par_iter().partial_cmp(b.par_iter());
    let seq_result = a.iter().partial_cmp(b.iter());

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_partial_cmp_late_nan_direct() {
    let a = vec![0.0, f64::NAN];
    let b = vec![1.0, 1.0];

    let result = a.par_iter().partial_cmp(b.par_iter());

    assert!(result == Some(::std::cmp::Ordering::Less));
}

#[test]
pub fn check_partial_cmp_late_nane_to_seq() {
    let a = vec![0.0, f64::NAN];
    let b = vec![1.0, 1.0];

    let par_result = a.par_iter().partial_cmp(b.par_iter());
    let seq_result = a.iter().partial_cmp(b.iter());

    assert_eq!(par_result, seq_result);
}


#[test]
pub fn check_eq_direct() {
    let a = (0..1024).into_par_iter();
    let b = (0..1024).into_par_iter();

    let result = a.eq(b);

    assert!(result);
}

#[test]
pub fn check_eq_to_seq() {
    let par_result = (0..1024).into_par_iter().eq((0..1024).into_par_iter());
    let seq_result = (0..1024).eq(0..1024);

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_ne_direct() {
    let a = (0..1024).into_par_iter();
    let b = (1..1024).into_par_iter();

    let result = a.ne(b);

    assert!(result);
}

#[test]
pub fn check_ne_to_seq() {
    let par_result = (0..1024).into_par_iter().ne((1..1024).into_par_iter());
    let seq_result = (0..1024).ne(1..1024);

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_lt_direct() {
    assert!((0..1024).into_par_iter().lt(1..1024));
    assert!(!(1..1024).into_par_iter().lt(0..1024));
}

pub fn check_lt_to_seq() {
    let par_result = (0..1024).into_par_iter().lt((1..1024).into_par_iter());
    let seq_result = (0..1024).lt(1..1024);

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_le_equal_direct() {
    assert!((0..1024).into_par_iter().le((0..1024).into_par_iter()));
}

pub fn check_le_equal_to_seq() {
    let par_result = (0..1024).into_par_iter().le((0..1024).into_par_iter());
    let seq_result = (0..1024).le(0..1024);

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_le_less_direct() {
    assert!((0..1024).into_par_iter().le((1..1024).into_par_iter()));
}

pub fn check_le_less_to_seq() {
    let par_result = (0..1024).into_par_iter().le((1..1024).into_par_iter());
    let seq_result = (0..1024).le(1..1024);

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_gt_direct() {
    assert!((1..1024).into_par_iter().gt((0..1024).into_par_iter()));
}

#[test]
pub fn check_gt_to_seq() {
    let par_result = (1..1024).into_par_iter().gt((0..1024).into_par_iter());
    let seq_result = (1..1024).gt(0..1024);

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_ge_equal_direct() {
    assert!((0..1024).into_par_iter().ge((0..1024).into_par_iter()));
}

#[test]
pub fn check_ge_equal_to_seq() {
    let par_result = (0..1024).into_par_iter().ge((0..1024).into_par_iter());
    let seq_result = (0..1024).ge(0..1024);

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_ge_greater_direct() {
    assert!((1..1024).into_par_iter().ge((0..1024).into_par_iter()));
}

#[test]
pub fn check_ge_greater_to_seq() {
    let par_result = (1..1024).into_par_iter().ge((0..1024).into_par_iter());
    let seq_result = (1..1024).ge(0..1024);

    assert_eq!(par_result, seq_result);
}

#[test]
pub fn check_zip() {
    let mut a: Vec<usize> = (0..1024).rev().collect();
    let b: Vec<usize> = (0..1024).collect();

    a.par_iter_mut()
        .zip(&b[..])
        .for_each(|(a, &b)| *a += b);

    assert!(a.iter().all(|&x| x == a.len() - 1));
}

#[test]
pub fn check_zip_into_par_iter() {
    let mut a: Vec<usize> = (0..1024).rev().collect();
    let b: Vec<usize> = (0..1024).collect();

    a.par_iter_mut()
     .zip(&b) // here we rely on &b iterating over &usize
     .for_each(|(a, &b)| *a += b);

    assert!(a.iter().all(|&x| x == a.len() - 1));
}

#[test]
pub fn check_zip_into_mut_par_iter() {
    let a: Vec<usize> = (0..1024).rev().collect();
    let mut b: Vec<usize> = (0..1024).collect();

    a.par_iter()
        .zip(&mut b)
        .for_each(|(&a, b)| *b += a);

    assert!(b.iter().all(|&x| x == b.len() - 1));
}

#[test]
pub fn check_zip_range() {
    let mut a: Vec<usize> = (0..1024).rev().collect();

    a.par_iter_mut()
        .zip(0usize..1024)
        .for_each(|(a, b)| *a += b);

    assert!(a.iter().all(|&x| x == a.len() - 1));
}

#[test]
pub fn check_range_split_at_overflow() {
    // Note, this split index overflows i8!
    let (left, right) = (-100i8..100).into_par_iter().split_at(150);
    let r1 = left.map(|i| i as i32).sum();
    let r2 = right.map(|i| i as i32).sum();
    assert_eq!(r1 + r2, -100);
}

#[test]
pub fn check_sum_filtered_ints() {
    let a: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let par_sum_evens = a.par_iter()
        .filter(|&x| (x & 1) == 0)
        .map(|&x| x)
        .sum();
    let seq_sum_evens = a.iter()
        .filter(|&x| (x & 1) == 0)
        .map(|&x| x)
        .fold(0, |a, b| a + b);
    assert_eq!(par_sum_evens, seq_sum_evens);
}

#[test]
pub fn check_sum_filtermap_ints() {
    let a: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let par_sum_evens = a.par_iter()
        .filter_map(|&x| if (x & 1) == 0 { Some(x as f32) } else { None })
        .sum();
    let seq_sum_evens = a.iter()
        .filter_map(|&x| if (x & 1) == 0 { Some(x as f32) } else { None })
        .fold(0.0, |a, b| a + b);
    assert_eq!(par_sum_evens, seq_sum_evens);
}

#[test]
pub fn check_flat_map_nested_ranges() {
    // FIXME -- why are precise type hints required on the integers here?

    let v = (0_i32..10)
        .into_par_iter()
        .flat_map(|i| (0_i32..10).into_par_iter().map(move |j| (i, j)))
        .map(|(i, j)| i * j)
        .sum();

    let w = (0_i32..10)
        .flat_map(|i| (0_i32..10).map(move |j| (i, j)))
        .map(|(i, j)| i * j)
        .fold(0, |i, j| i + j);

    assert_eq!(v, w);
}

#[test]
pub fn check_empty_flat_map_sum() {
    let a: Vec<i32> = (0..1024).collect();
    let empty = &a[..0];

    // empty on the inside
    let b = a.par_iter()
        .flat_map(|_| empty)
        .cloned()
        .sum();
    assert_eq!(b, 0);

    // empty on the outside
    let c = empty.par_iter()
        .flat_map(|_| a.par_iter())
        .cloned()
        .sum();
    assert_eq!(c, 0);
}

#[test]
pub fn check_chunks() {
    let a: Vec<i32> = vec![1, 5, 10, 4, 100, 3, 1000, 2, 10000, 1];
    let par_sum_product_pairs = a.par_chunks(2)
        .weight_max()
        .map(|c| c.iter().map(|&x| x).fold(1, |i, j| i * j))
        .sum();
    let seq_sum_product_pairs = a.chunks(2)
        .map(|c| c.iter().map(|&x| x).fold(1, |i, j| i * j))
        .fold(0, |i, j| i + j);
    assert_eq!(par_sum_product_pairs, 12345);
    assert_eq!(par_sum_product_pairs, seq_sum_product_pairs);

    let par_sum_product_triples: i32 = a.par_chunks(3)
        .weight_max()
        .map(|c| c.iter().map(|&x| x).fold(1, |i, j| i * j))
        .sum();
    let seq_sum_product_triples = a.chunks(3)
        .map(|c| c.iter().map(|&x| x).fold(1, |i, j| i * j))
        .fold(0, |i, j| i + j);
    assert_eq!(par_sum_product_triples, 5_0 + 12_00 + 2_000_0000 + 1);
    assert_eq!(par_sum_product_triples, seq_sum_product_triples);
}

#[test]
pub fn check_chunks_mut() {
    let mut a: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut b: Vec<i32> = a.clone();
    a.par_chunks_mut(2)
        .weight_max()
        .for_each(|c| c[0] = c.iter().map(|&x| x).fold(0, |i, j| i + j));
    b.chunks_mut(2)
        .map(|c| c[0] = c.iter().map(|&x| x).fold(0, |i, j| i + j))
        .count();
    assert_eq!(a, &[3, 2, 7, 4, 11, 6, 15, 8, 19, 10]);
    assert_eq!(a, b);

    let mut a: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut b: Vec<i32> = a.clone();
    a.par_chunks_mut(3)
        .weight_max()
        .for_each(|c| c[0] = c.iter().map(|&x| x).fold(0, |i, j| i + j));
    b.chunks_mut(3)
        .map(|c| c[0] = c.iter().map(|&x| x).fold(0, |i, j| i + j))
        .count();
    assert_eq!(a, &[6, 2, 3, 15, 5, 6, 24, 8, 9, 10]);
    assert_eq!(a, b);
}

#[test]
pub fn check_options() {
    let mut a = vec![None, Some(1), None, None, Some(2), Some(4)];

    assert_eq!(7, a.par_iter().flat_map(|opt| opt).cloned().sum());
    assert_eq!(7, a.par_iter().cloned().flat_map(|opt| opt).sum());

    a.par_iter_mut()
        .flat_map(|opt| opt)
        .for_each(|x| *x = *x * *x);

    assert_eq!(21, a.into_par_iter().flat_map(|opt| opt).sum());
}

#[test]
pub fn check_results() {
    let mut a = vec![Err(()), Ok(1), Err(()), Err(()), Ok(2), Ok(4)];

    assert_eq!(7, a.par_iter().flat_map(|res| res).cloned().sum());

    a.par_iter_mut()
        .flat_map(|res| res)
        .for_each(|x| *x = *x * *x);

    assert_eq!(21, a.into_par_iter().flat_map(|res| res).sum());
}

#[test]
pub fn check_binary_heap() {
    use std::collections::BinaryHeap;

    let a: BinaryHeap<i32> = (0..10).collect();

    assert_eq!(45, a.par_iter().cloned().sum());
    assert_eq!(45, a.into_par_iter().sum());
}

#[test]
pub fn check_btree_map() {
    use std::collections::BTreeMap;

    let mut a: BTreeMap<i32, i32> = (0..10).map(|i| (i, -i)).collect();

    assert_eq!(45, a.par_iter().map(|(&k, _)| k).sum());
    assert_eq!(-45, a.par_iter().map(|(_, &v)| v).sum());

    a.par_iter_mut().for_each(|(k, v)| *v += *k);

    assert_eq!(0, a.into_par_iter().map(|(_, v)| v).sum());
}

#[test]
pub fn check_btree_set() {
    use std::collections::BTreeSet;

    let a: BTreeSet<i32> = (0..10).collect();

    assert_eq!(45, a.par_iter().cloned().sum());
    assert_eq!(45, a.into_par_iter().sum());
}

#[test]
pub fn check_hash_map() {
    use std::collections::HashMap;

    let mut a: HashMap<i32, i32> = (0..10).map(|i| (i, -i)).collect();

    assert_eq!(45, a.par_iter().map(|(&k, _)| k).sum());
    assert_eq!(-45, a.par_iter().map(|(_, &v)| v).sum());

    a.par_iter_mut().for_each(|(k, v)| *v += *k);

    assert_eq!(0, a.into_par_iter().map(|(_, v)| v).sum());
}

#[test]
pub fn check_hash_set() {
    use std::collections::HashSet;

    let a: HashSet<i32> = (0..10).collect();

    assert_eq!(45, a.par_iter().cloned().sum());
    assert_eq!(45, a.into_par_iter().sum());
}

#[test]
pub fn check_linked_list() {
    use std::collections::LinkedList;

    let mut a: LinkedList<i32> = (0..10).collect();

    assert_eq!(45, a.par_iter().cloned().sum());

    a.par_iter_mut().for_each(|x| *x = -*x);

    assert_eq!(-45, a.into_par_iter().sum());
}

#[test]
pub fn check_vec_deque() {
    use std::collections::VecDeque;

    let mut a: VecDeque<i32> = (0..10).collect();

    // try to get it to wrap around
    a.drain(..5);
    a.extend(0..5);

    assert_eq!(45, a.par_iter().cloned().sum());

    a.par_iter_mut().for_each(|x| *x = -*x);

    assert_eq!(-45, a.into_par_iter().sum());
}

#[test]
pub fn check_chain() {
    let mut res = vec![];

    // stays indexed in the face of madness
    Some(0)
        .into_par_iter()
        .chain(Ok::<_, ()>(1))
        .chain(1..4)
        .chain(Err("huh?"))
        .chain(None)
        .chain(vec![5, 8, 13])
        .map(|x| (x as u8 + b'a') as char)
        .chain(vec!['x', 'y', 'z'])
        .zip((0i32..1000).into_par_iter().map(|x| -x))
        .enumerate()
        .map(|(a, (b, c))| (a, b, c))
        .weight_max()
        .chain(None)
        .collect_into(&mut res);

    assert_eq!(res,
               vec![(0, 'a', 0),
                    (1, 'b', -1),
                    (2, 'b', -2),
                    (3, 'c', -3),
                    (4, 'd', -4),
                    (5, 'f', -5),
                    (6, 'i', -6),
                    (7, 'n', -7),
                    (8, 'x', -8),
                    (9, 'y', -9),
                    (10, 'z', -10)]);

    // unindexed is ok too
    let res: Vec<i32> = Some(1i32)
        .into_par_iter()
        .chain((2i32..4)
            .into_par_iter()
            .chain(vec![5, 6, 7, 8, 9])
            .chain(Some((10, 100))
                .into_par_iter()
                .flat_map(|(a, b)| a..b))
            .filter(|x| x & 1 == 1))
        .weight_max()
        .collect();
    let other: Vec<i32> = (0..100)
        .filter(|x| x & 1 == 1)
        .collect();
    assert_eq!(res, other);
}


#[test]
pub fn check_count() {
    let c0 = (0_u32..24 * 1024).filter(|i| i % 2 == 0).count();
    let c1 = (0_u32..24 * 1024).into_par_iter().filter(|i| i % 2 == 0).count();
    let c2 = (0_u32..24 * 1024).into_par_iter().weight_max().filter(|i| i % 2 == 0).count();
    assert_eq!(c0, c1);
    assert_eq!(c1, c2);
}


#[test]
pub fn find_any() {
    let a: Vec<i32> = (0..1024).collect();

    assert!(a.par_iter().find_any(|&&x| x % 42 == 41).is_some());
    assert_eq!(a.par_iter().find_any(|&&x| x % 19 == 1 && x % 53 == 0),
               Some(&742_i32));
    assert_eq!(a.par_iter().find_any(|&&x| x < 0), None);

    assert!(a.par_iter().position_any(|&x| x % 42 == 41).is_some());
    assert_eq!(a.par_iter().position_any(|&x| x % 19 == 1 && x % 53 == 0),
               Some(742_usize));
    assert_eq!(a.par_iter().position_any(|&x| x < 0), None);

    assert!(a.par_iter().any(|&x| x > 1000));
    assert!(!a.par_iter().any(|&x| x < 0));

    assert!(!a.par_iter().all(|&x| x > 1000));
    assert!(a.par_iter().all(|&x| x >= 0));
}

#[test]
pub fn check_find_not_present() {
    let counter = AtomicUsize::new(0);
    let value: Option<i32> = (0_i32..2048)
        .into_par_iter()
        .find_any(|&p| {
            counter.fetch_add(1, Ordering::SeqCst);
            p >= 2048
        });
    assert!(value.is_none());
    assert!(counter.load(Ordering::SeqCst) == 2048); // should have visited every single one
}

#[test]
pub fn check_find_is_present() {
    let counter = AtomicUsize::new(0);
    let value: Option<i32> = (0_i32..2048)
        .into_par_iter()
        .find_any(|&p| {
            counter.fetch_add(1, Ordering::SeqCst);
            p >= 1024 && p < 1096
        });
    let q = value.unwrap();
    assert!(q >= 1024 && q < 1096);
    assert!(counter.load(Ordering::SeqCst) < 2048); // should not have visited every single one
}

#[test]
pub fn par_iter_collect() {
    let a: Vec<i32> = (0..1024).collect();
    let b: Vec<i32> = a.par_iter().map(|&i| i + 1).collect();
    let c: Vec<i32> = (0..1024).map(|i| i + 1).collect();
    assert_eq!(b, c);
}

#[test]
pub fn par_iter_collect_vecdeque() {
    let a: Vec<i32> = (0..1024).collect();
    let b: VecDeque<i32> = a.par_iter().cloned().collect();
    let c: VecDeque<i32> = a.iter().cloned().collect();
    assert_eq!(b, c);
}

#[test]
pub fn par_iter_collect_binaryheap() {
    let a: Vec<i32> = (0..1024).collect();
    let mut b: BinaryHeap<i32> = a.par_iter().cloned().collect();
    assert_eq!(b.peek(), Some(&1023));
    assert_eq!(b.len(), 1024);
    for n in (0..1024).rev() {
        assert_eq!(b.pop(), Some(n));
        assert_eq!(b.len() as i32, n);
    }
}

#[test]
pub fn par_iter_collect_hashmap() {
    let a: Vec<i32> = (0..1024).collect();
    let b: HashMap<i32, String> = a.par_iter().map(|&i| (i, format!("{}", i))).collect();
    assert_eq!(&b[&3], "3");
    assert_eq!(b.len(), 1024);
}

#[test]
pub fn par_iter_collect_hashset() {
    let a: Vec<i32> = (0..1024).collect();
    let b: HashSet<i32> = a.par_iter().cloned().collect();
    assert_eq!(b.len(), 1024);
}

#[test]
pub fn par_iter_collect_btreemap() {
    let a: Vec<i32> = (0..1024).collect();
    let b: BTreeMap<i32, String> = a.par_iter().map(|&i| (i, format!("{}", i))).collect();
    assert_eq!(&b[&3], "3");
    assert_eq!(b.len(), 1024);
}

#[test]
pub fn par_iter_collect_btreeset() {
    let a: Vec<i32> = (0..1024).collect();
    let b: BTreeSet<i32> = a.par_iter().cloned().collect();
    assert_eq!(b.len(), 1024);
}

#[test]
pub fn par_iter_collect_linked_list() {
    let a: Vec<i32> = (0..1024).collect();
    let b: LinkedList<_> = a.par_iter().map(|&i| (i, format!("{}", i))).collect();
    let c: LinkedList<_> = a.iter().map(|&i| (i, format!("{}", i))).collect();
    assert_eq!(b, c);
}

#[test]
pub fn par_iter_collect_linked_list_flat_map_filter() {
    let b: LinkedList<i32> = (0_i32..1024)
        .into_par_iter()
        .weight_max()
        .flat_map(|i| (0..i))
        .filter(|&i| i % 2 == 0)
        .collect();
    let c: LinkedList<i32> = (0_i32..1024).flat_map(|i| (0..i)).filter(|&i| i % 2 == 0).collect();
    assert_eq!(b, c);
}

#[test]
fn min_max() {
    let mut rng = XorShiftRng::from_seed([14159, 26535, 89793, 23846]);
    let a: Vec<i32> = rng.gen_iter().take(1024).collect();
    for i in 0..a.len() + 1 {
        let slice = &a[..i];
        assert_eq!(slice.par_iter().min(), slice.iter().min());
        assert_eq!(slice.par_iter().max(), slice.iter().max());
    }
}

#[test]
fn min_max_by() {
    let mut rng = XorShiftRng::from_seed([14159, 26535, 89793, 23846]);
    // Make sure there are duplicate keys, for testing sort stability
    let r: Vec<i32> = rng.gen_iter().take(512).collect();
    let a: Vec<(i32, u16)> = r.iter().chain(&r).cloned().zip(0..).collect();
    for i in 0..a.len() + 1 {
        let slice = &a[..i];
        assert_eq!(slice.par_iter().min_by_key(|x| x.0),
                   slice.iter().min_by_key(|x| x.0));
        assert_eq!(slice.par_iter().max_by_key(|x| x.0),
                   slice.iter().max_by_key(|x| x.0));
    }
}
