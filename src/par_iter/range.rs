use super::{IntoParallelIterator, ParallelIterator};
use super::len::ParallelLen;
use super::state::ParallelIteratorState;
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
            type Shared = ();
            type State = Self;

            fn state(self) -> (Self::Shared, Self::State) {
                ((), self)
            }
        }

        unsafe impl ParallelIteratorState for RangeIter<$t> {
            type Item = $t;
            type Shared = ();

            fn len(&mut self, _shared: &Self::Shared) -> ParallelLen {
                ParallelLen {
                    maximal_len: self.range.len(),
                    cost: self.range.len() as f64,
                    sparse: false,
                }
            }

            fn split_at(self, index: usize) -> (Self, Self) {
                assert!(index <= self.range.len());
                let mid = self.range.start + index as $t;
                let left = self.range.start .. mid;
                let right = mid .. self.range.end;
                (left.into_par_iter(), right.into_par_iter())
            }

            fn next(&mut self, _shared: &Self::Shared) -> Option<$t> {
                self.range.next()
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
