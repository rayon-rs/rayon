use super::*;
use super::internal::*;

fn is_bounded<T: ExactParallelIterator>(_: T) { }
fn is_exact<T: ExactParallelIterator>(_: T) { }
fn is_indexed<T: IndexedParallelIterator>(_: T) { }

#[test]
pub fn execute() {
    let a: Vec<i32> = (0..1024).collect();
    let mut b = vec![];
    a.par_iter()
     .map(|&i| i + 1)
     .collect_into(&mut b);
    let c: Vec<i32> = (0..1024).map(|i| i+1).collect();
    assert_eq!(b, c);
}

#[test]
pub fn execute_range() {
    let a = 0i32..1024;
    let mut b = vec![];
    a.into_par_iter()
     .map(|i| i + 1)
     .collect_into(&mut b);
    let c: Vec<i32> = (0..1024).map(|i| i+1).collect();
    assert_eq!(b, c);
}

#[test]
pub fn check_map_exact_and_bounded() {
    let a = [1, 2, 3];
    is_bounded(a.par_iter().map(|x| x));
    is_exact(a.par_iter().map(|x| x));
    is_indexed(a.par_iter().map(|x| x));
}

#[test]
pub fn map_reduce() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.par_iter()
              .weight_max()
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
    let r1 = a.par_iter()
              .weight_max()
              .map(|&i| i + 1)
              .reduce_with(|i, j| i + j);
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1.unwrap(), r2);
}

#[test]
pub fn map_reduce_with_identity() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.par_iter()
              .weight_max()
              .map(|&i| i + 1)
              .reduce_with_identity(0, |i, j| i + j);
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1, r2);
}

#[test]
pub fn map_reduce_weighted() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.par_iter()
              .map(|&i| i + 1)
              .weight(2.0)
              .reduce_with(|i, j| i + j);
    let r2 = a.iter()
              .map(|&i| i + 1)
              .fold(0, |a,b| a+b);
    assert_eq!(r1.unwrap(), r2);
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
        fn callback<P>(self, producer: P) where P: Producer<Item=(usize, &'a i32)> {
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
        fn callback<P>(self, producer: P) where P: Producer<Item=DropCounter<'a>> {
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
    let par_sum_evens =
        a.par_iter()
         .filter(|&x| (x & 1) == 0)
         .map(|&x| x)
         .sum();
    let seq_sum_evens =
        a.iter()
         .filter(|&x| (x & 1) == 0)
         .map(|&x| x)
         .fold(0, |a,b| a+b);
    assert_eq!(par_sum_evens, seq_sum_evens);
}

#[test]
pub fn check_sum_filtermap_ints() {
    let a: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let par_sum_evens =
        a.par_iter()
         .filter_map(|&x| if (x & 1) == 0 {Some(x as f32)} else {None})
         .sum();
    let seq_sum_evens =
        a.iter()
         .filter_map(|&x| if (x & 1) == 0 {Some(x as f32)} else {None})
         .fold(0.0, |a,b| a+b);
    assert_eq!(par_sum_evens, seq_sum_evens);
}

#[test]
pub fn check_flat_map_nested_ranges() {
    // FIXME -- why are precise type hints required on the integers here?

    let v =
        (0_i32..10).into_par_iter()
                   .flat_map(|i| (0_i32..10).into_par_iter().map(move |j| (i, j)))
                   .map(|(i, j)| i * j)
                   .sum();

    let w =
        (0_i32..10).flat_map(|i| (0_i32..10).map(move |j| (i, j)))
                   .map(|(i, j)| i * j)
                   .fold(0, |i, j| i + j);

    assert_eq!(v, w);
}

