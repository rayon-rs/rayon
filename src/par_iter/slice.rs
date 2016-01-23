use super::{ParallelIterator, IntoParallelIterator};
use super::len::ParallelLen;
use super::state::ParallelIteratorState;

pub struct SliceIter<'map, T: 'map + Sync> {
    slice: &'map [T]
}

impl<'map, T: Sync> IntoParallelIterator for &'map [T] {
    type Item = &'map T;
    type Iter = SliceIter<'map, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIter { slice: self }
    }
}

impl<'map, T: Sync> ParallelIterator for SliceIter<'map, T> {
    type Item = &'map T;
    type Shared = ();
    type State = Self;

    fn state(self) -> (Self::Shared, Self::State) {
        ((), self)
    }
}

unsafe impl<'map, T: Sync> ParallelIteratorState for SliceIter<'map, T> {
    type Item = &'map T;
    type Shared = ();

    fn len(&mut self, _shared: &Self::Shared) -> ParallelLen {
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
