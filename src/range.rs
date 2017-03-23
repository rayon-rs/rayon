//! This module contains the parallel iterator types for ranges
//! (`Range<T>`); this is the type for values created by a `a..b`
//! expression. You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use iter::*;
use iter::internal::*;
use std::ops::Range;

/// Parallel iterator over a range
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

impl<T> IntoIterator for Iter<T>
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

            fn opt_len(&mut self) -> Option<usize> {
                Some(self.len())
            }
        }

        impl BoundedParallelIterator for Iter<$t> {
            fn upper_bound(&mut self) -> usize {
                ExactParallelIterator::len(self)
            }

            fn drive<C>(self, consumer: C) -> C::Result
                where C: Consumer<Self::Item>
            {
                bridge(self, consumer)
            }
        }

        impl ExactParallelIterator for Iter<$t> {
            fn len(&mut self) -> usize {
                self.range.len()
            }
        }

        impl IndexedParallelIterator for Iter<$t> {
            fn with_producer<CB>(self, callback: CB) -> CB::Output
                where CB: ProducerCallback<Self::Item>
            {
                callback.callback(self)
            }
        }

        impl Producer for Iter<$t> {

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
                (Iter { range: left }, Iter { range: right })
            }
        }
    }
}

macro_rules! unindexed_range_impl {
    ( $t:ty ) => {
        impl Iter<$t> {
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
                bridge_unindexed(self, consumer)
            }
        }

        impl UnindexedProducer for Iter<$t> {
            type Item = $t;

            fn split(mut self) -> (Self, Option<Self>) {
                let index = self.len() / 2;
                if index > 0 {
                    let mid = self.range.start.wrapping_add(index as $t);
                    let right = mid .. self.range.end;
                    self.range.end = mid;
                    (self, Some(Iter { range: right }))
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
