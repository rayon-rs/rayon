use std::sync::atomic::{AtomicUsize, Ordering};

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
pub fn map_sum() {
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
pub fn map_reduce() {
    let a: Vec<i32> = (0..1024).collect();
    let r1 = a.par_iter()
              .weight_max()
              .map(|&i| i + 1)
              .reduce(|| 0, |i, j| i + j);
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
    let r1 = (0_i32..32).into_par_iter()
                        .weight_max()
                        .fold(|| vec![], |mut v, e| { v.push(e); v })
                        .map(|v| vec![v])
                        .reduce_with(|mut v_a, v_b| { v_a.extend(v_b); v_a });
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
pub fn check_inspect() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let a = AtomicUsize::new(0);
    let b = (0_usize..1024).into_par_iter()
        .weight_max()
        .inspect(|&i| { a.fetch_add(i, Ordering::Relaxed); })
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
    let par_sum_product_pairs =
         a.par_chunks(2)
          .weight_max()
          .map(|c| c.iter().map(|&x|x).fold(1, |i, j| i*j))
          .sum();
    let seq_sum_product_pairs =
         a.chunks(2)
          .map(|c| c.iter().map(|&x|x).fold(1, |i, j| i*j))
          .fold(0, |i, j| i+j);
    assert_eq!(par_sum_product_pairs, 12345);
    assert_eq!(par_sum_product_pairs, seq_sum_product_pairs);

    let par_sum_product_triples: i32 =
         a.par_chunks(3)
          .weight_max()
          .map(|c| c.iter().map(|&x|x).fold(1, |i, j| i*j))
          .sum();
    let seq_sum_product_triples =
         a.chunks(3)
          .map(|c| c.iter().map(|&x|x).fold(1, |i, j| i*j))
          .fold(0, |i, j| i+j);
    assert_eq!(par_sum_product_triples, 5_0 + 12_00 + 2_000_0000 + 1);
    assert_eq!(par_sum_product_triples, seq_sum_product_triples);
}

#[test]
pub fn check_chunks_mut() {
    let mut a: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut b: Vec<i32> = a.clone();
    a.par_chunks_mut(2)
        .weight_max()
        .for_each(|c| c[0] = c.iter().map(|&x|x).fold(0, |i, j| i+j));
    b.chunks_mut(2)
        .map(     |c| c[0] = c.iter().map(|&x|x).fold(0, |i, j| i+j))
        .count();
    assert_eq!(a, &[3, 2, 7, 4, 11, 6, 15, 8, 19, 10]);
    assert_eq!(a, b);

    let mut a: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut b: Vec<i32> = a.clone();
    a.par_chunks_mut(3)
        .weight_max()
        .for_each(|c| c[0] = c.iter().map(|&x|x).fold(0, |i, j| i+j));
    b.chunks_mut(3)
        .map(     |c| c[0] = c.iter().map(|&x|x).fold(0, |i, j| i+j))
        .count();
    assert_eq!(a, &[6, 2, 3, 15, 5, 6, 24, 8, 9, 10]);
    assert_eq!(a, b);
}

#[test]
pub fn check_options() {
    let mut a = vec![None, Some(1), None, None, Some(2), Some(4)];

    assert_eq!(7, a.par_iter().flat_map(|opt| opt).cloned().sum());
    assert_eq!(7, a.par_iter().cloned().flat_map(|opt| opt).sum());

    a.par_iter_mut().flat_map(|opt| opt)
        .for_each(|x| *x = *x * *x);

    assert_eq!(21, a.into_par_iter().flat_map(|opt| opt).sum());
}

#[test]
pub fn check_results() {
    let mut a = vec![Err(()), Ok(1), Err(()), Err(()), Ok(2), Ok(4)];

    assert_eq!(7, a.par_iter().flat_map(|res| res).cloned().sum());

    a.par_iter_mut().flat_map(|res| res)
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
    Some(0).into_par_iter()
        .chain(Ok::<_,()>(1))
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
        .collect_into(&mut res);

    assert_eq!(res, vec![(0, 'a', 0),
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
    let sum = Some(1i32).into_par_iter()
        .chain((2i32..4).into_par_iter()
                .chain(vec![5, 6, 7, 8, 9])
                .chain(Some((10, 100)).into_par_iter()
                       .flat_map(|(a, b)| a..b))
                .filter(|x| x & 1 == 1))
        .weight_max()
        .sum();
    assert_eq!(sum, 2500);
}


#[test]
pub fn check_count() {
    let c0 = (0_u32..24*1024).filter(|i| i % 2 == 0).count();
    let c1 = (0_u32..24*1024).into_par_iter().filter(|i| i % 2 == 0).count();
    let c2 = (0_u32..24*1024).into_par_iter().weight_max().filter(|i| i % 2 == 0).count();
    assert_eq!(c0, c1);
    assert_eq!(c1, c2);
}


#[test]
pub fn find_any() {
    let a: Vec<i32> = (0..1024).collect();

    assert!(a.par_iter().find_any(|&&x| x % 42 == 41).is_some());
    assert_eq!(a.par_iter().find_any(|&&x| x % 19 == 1 && x % 53 == 0), Some(&742_i32));
    assert_eq!(a.par_iter().find_any(|&&x| x < 0), None);

    assert!(a.par_iter().position_any(|&x| x % 42 == 41).is_some());
    assert_eq!(a.par_iter().position_any(|&x| x % 19 == 1 && x % 53 == 0), Some(742_usize));
    assert_eq!(a.par_iter().position_any(|&x| x < 0), None);

    assert!(a.par_iter().any(|&x| x > 1000));
    assert!(!a.par_iter().any(|&x| x < 0));

    assert!(!a.par_iter().all(|&x| x > 1000));
    assert!(a.par_iter().all(|&x| x >= 0));
}

#[test]
pub fn check_find_not_present() {
    let counter = AtomicUsize::new(0);
    let value: Option<i32> =
        (0_i32..2048)
        .into_par_iter()
        .find_any(|&p| { counter.fetch_add(1, Ordering::SeqCst); p >= 2048 });
    assert!(value.is_none());
    assert!(counter.load(Ordering::SeqCst) == 2048); // should have visited every single one
}

#[test]
pub fn check_find_is_present() {
    let counter = AtomicUsize::new(0);
    let value: Option<i32> =
        (0_i32..2048)
        .into_par_iter()
        .find_any(|&p| { counter.fetch_add(1, Ordering::SeqCst); p >= 1024 && p < 1096 });
    let q = value.unwrap();
    assert!(q >= 1024 && q < 1096);
    assert!(counter.load(Ordering::SeqCst) < 2048); // should not have visited every single one
}
