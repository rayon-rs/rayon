use super::*;
use super::len::ParallelLen;
use super::state::ParallelIteratorState;
use std::mem;

pub struct SliceIterMut<'r, T: 'r + Send> {
    slice: &'r mut [T]
}

impl<'r, T: Send> IntoParallelIterator for &'r mut [T] {
    type Item = &'r mut T;
    type Iter = SliceIterMut<'r, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIterMut { slice: self }
    }
}

impl<'r, T: Send + 'r> IntoParallelRefMutIterator<'r> for [T] {
    type Item = T;
    type Iter = SliceIterMut<'r, T>;

    fn par_iter_mut(&'r mut self) -> Self::Iter {
        self.into_par_iter()
    }
}

impl<'r, T: Send> ParallelIterator for SliceIterMut<'r, T> {
    type Item = &'r mut T;
    type Shared = ();
    type State = Self;

    fn state(self) -> (Self::Shared, Self::State) {
        ((), self)
    }
}

unsafe impl<'r, T: Send> BoundedParallelIterator for SliceIterMut<'r, T> { }

unsafe impl<'r, T: Send> ExactParallelIterator for SliceIterMut<'r, T> { }

unsafe impl<'r, T: Send> ParallelIteratorState for SliceIterMut<'r, T> {
    type Item = &'r mut T;
    type Shared = ();

    fn len(&mut self, _shared: &Self::Shared) -> ParallelLen {
        ParallelLen {
            maximal_len: self.slice.len(),
            cost: self.slice.len() as f64,
            sparse: false,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at_mut(index);
        (left.into_par_iter(), right.into_par_iter())
    }

    fn next(&mut self, _shared: &Self::Shared) -> Option<&'r mut T> {
        let slice = mem::replace(&mut self.slice, &mut []); // FIXME rust-lang/rust#10520
        slice.split_first_mut()
             .map(|(head, tail)| {
                 self.slice = tail;
                 head
             })
    }
}
