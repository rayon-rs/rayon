use super::{IntoParallelIterator, ParallelIterator, ParallelIteratorState, ParallelLen};
use std::ops::Range;

pub struct RangeIter<T> {
    slice: Range<T>
}

pub trait ParallelRange: Eq + Send {
    fn len(start: Self, stop: Self) -> usize;
    fn mid(start: Self, stop: Self) -> Self;
    fn increment(start: Self) -> Self;
}

impl<T: ParallelRange> IntoParallelIterator for Range<T> {
    type Item = T;
    type Iter = RangeIter<'map, T>;

    fn into_par_iter(self) -> Self::Iter {
        RangeIter { slice: self }
    }
}

impl<T: ParallelRange> ParallelIterator for RangeIter<'map, T> {
    type Item = T;
    type Shared = ();
    type State = Self;

    fn state(self) -> (Self::Shared, Self::State) {
        ((), self)
    }
}

impl<T: ParallelRange> ParallelIteratorState for RangeIter<'map, T> {
    type Item = T;
    type Shared = ();

    fn len(&mut self) -> ParallelLen {
        ParallelLen {
            maximal_len: self.slice.len(),
            cost: self.slice.len() as f64,
            sparse: false,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at(index);
        (left.into_par_iter(), right.into_par_iter())
    }

    fn for_each<OP>(self, _shared: &Self::Shared, mut op: OP)
        where OP: FnMut(&'map T)
    {
        for item in self.slice {
            op(item);
        }
    }
}
