//! Parallel iterator types for [ranges][std::range],
//! the type for values created by `a..b` expressions
//!
//! You will rarely need to interact with this module directly unless you have
//! need to name one of the iterator types.
//!
//! ```
//! use rayon::prelude::*;
//!
//! let r = (0..100u64).into_par_iter()
//!                    .sum();
//!
//! // compare result with sequential calculation
//! assert_eq!((0..100).sum::<u64>(), r);
//! ```
//!
//! [std::range]: https://doc.rust-lang.org/core/ops/struct.Range.html

use iter::plumbing::*;
use iter::*;
use std::ops::Range;
use std::usize;

/// Parallel iterator over a range, implemented for all integer types.
///
/// **Note:** The `zip` operation requires `IndexedParallelIterator`
/// which is not implemented for `u64`, `i64`, `u128`, or `i128`.
///
/// ```
/// use rayon::prelude::*;
///
/// let p = (0..25usize).into_par_iter()
///                   .zip(0..25usize)
///                   .filter(|&(x, y)| x % 5 == 0 || y % 5 == 0)
///                   .map(|(x, y)| x * y)
///                   .sum::<usize>();
///
/// let s = (0..25usize).zip(0..25)
///                   .filter(|&(x, y)| x % 5 == 0 || y % 5 == 0)
///                   .map(|(x, y)| x * y)
///                   .sum();
///
/// assert_eq!(p, s);
/// ```
#[derive(Debug, Clone)]
pub struct Iter<T> {
    range: Range<T>,
}

impl<T> IntoParallelIterator for Range<T>
where
    Iter<T>: ParallelIterator,
{
    type Item = <Iter<T> as ParallelIterator>::Item;
    type Iter = Iter<T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { range: self }
    }
}

struct IterProducer<T> {
    range: Range<T>,
}

impl<T> IntoIterator for IterProducer<T>
where
    Range<T>: Iterator,
{
    type Item = <Range<T> as Iterator>::Item;
    type IntoIter = Range<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.range
    }
}

macro_rules! indexed_range_impl {
    ( $t:ty ) => {
        impl ParallelIterator for Iter<$t> {
            type Item = $t;

            fn drive_unindexed<C>(self, consumer: C) -> C::Result
            where
                C: UnindexedConsumer<Self::Item>,
            {
                bridge(self, consumer)
            }

            fn opt_len(&self) -> Option<usize> {
                Some(self.len())
            }
        }

        impl IndexedParallelIterator for Iter<$t> {
            fn drive<C>(self, consumer: C) -> C::Result
            where
                C: Consumer<Self::Item>,
            {
                bridge(self, consumer)
            }

            fn len(&self) -> usize {
                self.range.len()
            }

            fn with_producer<CB>(self, callback: CB) -> CB::Output
            where
                CB: ProducerCallback<Self::Item>,
            {
                callback.callback(IterProducer { range: self.range })
            }
        }

        impl Producer for IterProducer<$t> {
            type Item = <Range<$t> as Iterator>::Item;
            type IntoIter = Range<$t>;
            fn into_iter(self) -> Self::IntoIter {
                self.range
            }

            fn split_at(self, index: usize) -> (Self, Self) {
                assert!(index <= self.range.len());
                // For signed $t, the length and requested index could be greater than $t::MAX, and
                // then `index as $t` could wrap to negative, so wrapping_add is necessary.
                let mid = self.range.start.wrapping_add(index as $t);
                let left = self.range.start..mid;
                let right = mid..self.range.end;
                (IterProducer { range: left }, IterProducer { range: right })
            }
        }
    };
}

trait UnindexedRangeLen<L> {
    fn len(&self) -> L;
}

macro_rules! unindexed_range_impl {
    ( $t:ty, $len_t:ty ) => {
        impl UnindexedRangeLen<$len_t> for Range<$t> {
            fn len(&self) -> $len_t {
                let &Range { start, end } = self;
                if end > start {
                    end.wrapping_sub(start) as $len_t
                } else {
                    0
                }
            }
        }

        impl ParallelIterator for Iter<$t> {
            type Item = $t;

            fn drive_unindexed<C>(self, consumer: C) -> C::Result
            where
                C: UnindexedConsumer<Self::Item>,
            {
                #[inline]
                fn offset(start: $t) -> impl Fn(usize) -> $t {
                    move |i| start.wrapping_add(i as $t)
                }

                if let Some(len) = self.opt_len() {
                    // Drive this in indexed mode for better `collect`.
                    (0..len)
                        .into_par_iter()
                        .map(offset(self.range.start))
                        .drive(consumer)
                } else {
                    bridge_unindexed(IterProducer { range: self.range }, consumer)
                }
            }

            fn opt_len(&self) -> Option<usize> {
                let len = self.range.len();
                if len <= usize::MAX as $len_t {
                    Some(len as usize)
                } else {
                    None
                }
            }
        }

        impl UnindexedProducer for IterProducer<$t> {
            type Item = $t;

            fn split(mut self) -> (Self, Option<Self>) {
                let index = self.range.len() / 2;
                if index > 0 {
                    let mid = self.range.start.wrapping_add(index as $t);
                    let right = mid..self.range.end;
                    self.range.end = mid;
                    (self, Some(IterProducer { range: right }))
                } else {
                    (self, None)
                }
            }

            fn fold_with<F>(self, folder: F) -> F
            where
                F: Folder<Self::Item>,
            {
                folder.consume_iter(self)
            }
        }
    };
}

// all Range<T> with ExactSizeIterator
indexed_range_impl! {u8}
indexed_range_impl! {u16}
indexed_range_impl! {u32}
indexed_range_impl! {usize}
indexed_range_impl! {i8}
indexed_range_impl! {i16}
indexed_range_impl! {i32}
indexed_range_impl! {isize}

// other Range<T> with just Iterator
unindexed_range_impl! {u64, u64}
unindexed_range_impl! {i64, u64}
unindexed_range_impl! {u128, u128}
unindexed_range_impl! {i128, u128}

#[test]
fn check_range_split_at_overflow() {
    // Note, this split index overflows i8!
    let producer = IterProducer { range: -100i8..100 };
    let (left, right) = producer.split_at(150);
    let r1: i32 = left.range.map(i32::from).sum();
    let r2: i32 = right.range.map(i32::from).sum();
    assert_eq!(r1 + r2, -100);
}

#[test]
fn test_i128_len_doesnt_overflow() {
    use std::{i128, u128};

    // Using parse because some versions of rust don't allow long literals
    let octillion: i128 = "1000000000000000000000000000".parse().unwrap();
    let producer = IterProducer {
        range: 0..octillion,
    };

    assert_eq!(octillion as u128, producer.range.len());
    assert_eq!(octillion as u128, (0..octillion).len());
    assert_eq!(2 * octillion as u128, (-octillion..octillion).len());

    assert_eq!(u128::MAX, (i128::MIN..i128::MAX).len());
}

#[test]
fn test_u64_opt_len() {
    use std::{u64, usize};
    assert_eq!(Some(100), (0..100u64).into_par_iter().opt_len());
    assert_eq!(
        Some(usize::MAX),
        (0..usize::MAX as u64).into_par_iter().opt_len()
    );
    if (usize::MAX as u64) < u64::MAX {
        assert_eq!(
            None,
            (0..(usize::MAX as u64).wrapping_add(1))
                .into_par_iter()
                .opt_len()
        );
        assert_eq!(None, (0..u64::MAX).into_par_iter().opt_len());
    }
}

#[test]
fn test_u128_opt_len() {
    use std::{u128, usize};
    assert_eq!(Some(100), (0..100u128).into_par_iter().opt_len());
    assert_eq!(
        Some(usize::MAX),
        (0..usize::MAX as u128).into_par_iter().opt_len()
    );
    assert_eq!(None, (0..1 + usize::MAX as u128).into_par_iter().opt_len());
    assert_eq!(None, (0..u128::MAX).into_par_iter().opt_len());
}

// `usize as i64` can overflow, so make sure to wrap it appropriately
// when using the `opt_len` "indexed" mode.
#[test]
#[cfg(target_pointer_width = "64")]
fn test_usize_i64_overflow() {
    use std::i64;
    use ThreadPoolBuilder;

    let iter = (-2..i64::MAX).into_par_iter();
    assert_eq!(iter.opt_len(), Some(i64::MAX as usize + 2));

    // always run with multiple threads to split into, or this will take forever...
    let pool = ThreadPoolBuilder::new().num_threads(8).build().unwrap();
    pool.install(|| assert_eq!(iter.find_last(|_| true), Some(i64::MAX - 1)));
}
