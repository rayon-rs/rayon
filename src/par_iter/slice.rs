use super::{ParallelIterator, IntoParallelIterator, IntoParallelRefIterator};
use super::len::ParallelLen;
use super::state::ParallelIteratorState;

pub struct SliceIter<'r, T: 'r + Sync> {
    slice: &'r [T]
}

impl<'r, T: Sync> IntoParallelIterator for &'r [T] {
    type Item = &'r T;
    type Iter = SliceIter<'r, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIter { slice: self }
    }
}

impl<'r, T: Sync + 'r> IntoParallelRefIterator<'r> for [T] {
    type Item = T;
    type Iter = SliceIter<'r, T>;

    fn par_iter(&'r self) -> Self::Iter {
        self.into_par_iter()
    }
}

impl<'r, T: Sync> ParallelIterator for SliceIter<'r, T> {
    type Item = &'r T;
    type Shared = ();
    type State = Self;

    fn state(self) -> (Self::Shared, Self::State) {
        ((), self)
    }
}

unsafe impl<'r, T: Sync> ParallelIteratorState for SliceIter<'r, T> {
    type Item = &'r T;
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
        where OP: FnMut(&'r T)
    {
        for item in self.slice {
            op(item);
        }
    }
}
