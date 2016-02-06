use super::*;
use super::internal::*;
use std::ops::Range;

pub struct RangeIter<T> {
    range: Range<T>
}

macro_rules! range_impl {
    ( $t:ty ) => {
        impl IntoParallelIterator for Range<$t> {
            type Item = $t;
            type Iter = RangeIter<$t>;

            fn into_par_iter(self) -> Self::Iter {
                RangeIter { range: self }
            }
        }

        impl ParallelIterator for RangeIter<$t> {
            type Item = $t;

            fn drive_unindexed<C>(self, consumer: C) -> C::Result
                where C: UnindexedConsumer<Self::Item>
            {
                bridge(self, consumer)
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
                self.range.len() as usize
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

        impl IntoIterator for RangeIter<$t> {
            type Item = $t;
            type IntoIter = Range<$t>;

            fn into_iter(self) -> Self::IntoIter {
                self.range
            }
        }
    }
}

// all Range<T> with ExactSizeIterator
range_impl!{u8}
range_impl!{u16}
range_impl!{u32}
range_impl!{usize}
range_impl!{i8}
range_impl!{i16}
range_impl!{i32}
range_impl!{isize}
