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

use iter::*;
use iter::plumbing::*;
use std::ops::Range;

/// Parallel iterator over a range, implemented for all integer types.
///
/// **Note:** The `zip` operation requires `IndexedParallelIterator`
/// which is not implemented for `u64` or `i64`.
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
    where Iter<T>: ParallelIterator
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
    where Range<T>: Iterator
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
                where C: UnindexedConsumer<Self::Item>
            {
                bridge(self, consumer)
            }

            fn opt_len(&self) -> Option<usize> {
                Some(self.len())
            }
        }

        impl IndexedParallelIterator for Iter<$t> {
            fn drive<C>(self, consumer: C) -> C::Result
                where C: Consumer<Self::Item>
            {
                bridge(self, consumer)
            }

            fn len(&self) -> usize {
                self.range.len()
            }

            fn with_producer<CB>(self, callback: CB) -> CB::Output
                where CB: ProducerCallback<Self::Item>
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
                let left = self.range.start .. mid;
                let right = mid .. self.range.end;
                (IterProducer { range: left }, IterProducer { range: right })
            }
        }
    }
}

macro_rules! unindexed_range_impl {
    ( $t:ty ) => {
        impl IterProducer<$t> {
            fn len(&self) -> u64 {
                let Range { start, end } = self.range;
                if end > start {
                    end.wrapping_sub(start) as u64
                } else {
                    0
                }
            }
        }

        impl ParallelIterator for Iter<$t> {
            type Item = $t;

            fn drive_unindexed<C>(self, consumer: C) -> C::Result
                where C: UnindexedConsumer<Self::Item>
            {
                bridge_unindexed(IterProducer { range: self.range }, consumer)
            }
        }

        impl UnindexedProducer for IterProducer<$t> {
            type Item = $t;

            fn split(mut self) -> (Self, Option<Self>) {
                let index = self.len() / 2;
                if index > 0 {
                    let mid = self.range.start.wrapping_add(index as $t);
                    let right = mid .. self.range.end;
                    self.range.end = mid;
                    (self, Some(IterProducer { range: right }))
                } else {
                    (self, None)
                }
            }

            fn fold_with<F>(self, folder: F) -> F
                where F: Folder<Self::Item>
            {
                folder.consume_iter(self)
            }
        }
    }
}

// all Range<T> with ExactSizeIterator
indexed_range_impl!{u8}
indexed_range_impl!{u16}
indexed_range_impl!{u32}
indexed_range_impl!{usize}
indexed_range_impl!{i8}
indexed_range_impl!{i16}
indexed_range_impl!{i32}
indexed_range_impl!{isize}

// other Range<T> with just Iterator
unindexed_range_impl!{u64}
unindexed_range_impl!{i64}


#[test]
pub fn check_range_split_at_overflow() {
    // Note, this split index overflows i8!
    let producer = IterProducer { range: -100i8..100 };
    let (left, right) = producer.split_at(150);
    let r1: i32 = left.range.map(|i| i as i32).sum();
    let r2: i32 = right.range.map(|i| i as i32).sum();
    assert_eq!(r1 + r2, -100);
}
