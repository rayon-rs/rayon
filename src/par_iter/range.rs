use super::internal::*;
use super::*;
use std::ops::Range;

pub struct RangeIter<T> {
    range: Range<T>,
}

impl<T> IntoParallelIterator for Range<T>
    where RangeIter<T>: ParallelIterator
{
    type Item = <RangeIter<T> as ParallelIterator>::Item;
    type Iter = RangeIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        RangeIter { range: self }
    }
}

impl<T> IntoIterator for RangeIter<T>
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
        impl ParallelIterator for RangeIter<$t> {
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

        impl BoundedParallelIterator for RangeIter<$t> {
            fn upper_bound(&mut self) -> usize {
                ExactParallelIterator::len(self)
            }

            fn drive<C>(self, consumer: C) -> C::Result
                where C: Consumer<Self::Item>
            {
                bridge(self, consumer)
            }
        }

        impl ExactParallelIterator for RangeIter<$t> {
            fn len(&mut self) -> usize {
                self.range.len()
            }
        }

        impl IndexedParallelIterator for RangeIter<$t> {
            fn with_producer<CB>(self, callback: CB) -> CB::Output
                where CB: ProducerCallback<Self::Item>
            {
                callback.callback(self)
            }
        }

        impl Producer for RangeIter<$t> {
            fn cost(&mut self, len: usize) -> f64 {
                len as f64
            }

            fn split_at(self, index: usize) -> (Self, Self) {
                assert!(index <= self.range.len());
                // For signed $t, the length and requested index could be greater than $t::MAX, and
                // then `index as $t` could wrap to negative, so wrapping_add is necessary.
                let mid = self.range.start.wrapping_add(index as $t);
                let left = self.range.start .. mid;
                let right = mid .. self.range.end;
                (RangeIter { range: left }, RangeIter { range: right })
            }
        }
    }
}

macro_rules! unindexed_range_impl {
    ( $t:ty ) => {
        impl RangeIter<$t> {
            fn len(&self) -> u64 {
                let Range { start, end } = self.range;
                if end > start {
                    end.wrapping_sub(start) as u64
                } else {
                    0
                }
            }
        }

        impl ParallelIterator for RangeIter<$t> {
            type Item = $t;

            fn drive_unindexed<C>(self, consumer: C) -> C::Result
                where C: UnindexedConsumer<Self::Item>
            {
                bridge_unindexed(self, consumer)
            }
        }

        impl UnindexedProducer for RangeIter<$t> {
            fn can_split(&self) -> bool {
                self.len() > 1
            }

            fn split(self) -> (Self, Self) {
                let index = self.len() / 2;
                let mid = self.range.start.wrapping_add(index as $t);
                let left = self.range.start .. mid;
                let right = mid .. self.range.end;
                (RangeIter { range: left }, RangeIter { range: right })
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
