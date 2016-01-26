use super::*;
use super::state::*;
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

            fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                              consumer: C,
                                                                              shared: &'c C::Shared)
                                                                              -> C::Result {
                bridge(self, consumer, &shared)
            }
        }

        unsafe impl BoundedParallelIterator for RangeIter<$t> {
            fn upper_bound(&mut self) -> usize {
                ExactParallelIterator::len(self)
            }

            fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                           consumer: C,
                                                           shared: &'c C::Shared)
                                                           -> C::Result {
                bridge(self, consumer, &shared)
            }
        }

        unsafe impl ExactParallelIterator for RangeIter<$t> {
            fn len(&mut self) -> usize {
                self.range.len() as usize
            }
        }

        impl IndexedParallelIterator for RangeIter<$t> {
            type Producer = Self;

            fn into_producer(self) -> (Self::Producer, <Self::Producer as Producer>::Shared) {
                (self, ())
            }
        }

        impl Producer for RangeIter<$t> {
            type Item = $t;
            type Shared = ();

            fn cost(&mut self, _: &Self::Shared, len: usize) -> f64 {
                len as f64
            }

            unsafe fn split_at(self, index: usize) -> (Self, Self) {
                assert!(index <= self.range.len());
                // For signed $t, the length and requested index could be greater than $t::MAX, and
                // then `index as $t` could wrap to negative, so wrapping_add is necessary.
                let mid = self.range.start.wrapping_add(index as $t);
                let left = self.range.start .. mid;
                let right = mid .. self.range.end;
                (RangeIter { range: left }, RangeIter { range: right })
            }

            unsafe fn produce(&mut self, _: &()) -> $t {
                self.range.next().unwrap()
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
