use super::*;
use super::len::ParallelLen;
use super::state::*;
use std::mem;

pub struct SliceIterMut<'data, T: 'data + Send> {
    slice: &'data mut [T]
}

impl<'data, T: Send> IntoParallelIterator for &'data mut [T] {
    type Item = &'data mut T;
    type Iter = SliceIterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIterMut { slice: self }
    }
}

impl<'data, T: Send + 'data> IntoParallelRefMutIterator<'data> for [T] {
    type Item = T;
    type Iter = SliceIterMut<'data, T>;

    fn par_iter_mut(&'data mut self) -> Self::Iter {
        self.into_par_iter()
    }
}

impl<'data, T: Send> ParallelIterator for SliceIterMut<'data, T> {
    type Item = &'data mut T;
    type Shared = ();
    type State = Self;

    fn drive<C: Consumer<Item=Self::Item>>(self, consumer: C) -> C::Result {
        unimplemented!()
    }

    fn state(self) -> (Self::Shared, Self::State) {
        ((), self)
    }
}

unsafe impl<'data, T: Send> BoundedParallelIterator for SliceIterMut<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }
}

unsafe impl<'data, T: Send> ExactParallelIterator for SliceIterMut<'data, T> {
    fn len(&mut self) -> usize {
        self.slice.len() as usize
    }
}

impl<'data, T: Send> PullParallelIterator for SliceIterMut<'data, T> {
}

unsafe impl<'data, T: Send> ParallelIteratorState for SliceIterMut<'data, T> {
    type Item = &'data mut T;
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

    fn next(&mut self, _shared: &Self::Shared) -> Option<&'data mut T> {
        let slice = mem::replace(&mut self.slice, &mut []); // FIXME rust-lang/rust#10520
        slice.split_first_mut()
             .map(|(head, tail)| {
                 self.slice = tail;
                 head
             })
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct SliceMutProducer<'data, T: 'data + Send> {
    slice: &'data mut [T]
}

impl<'data, T: 'data + Send> Producer for SliceMutProducer<'data, T>
{
    type Item = &'data mut T;
    type Shared = ();

    unsafe fn cost(&mut self, shared: &Self::Shared, items: usize) -> f64 {
        items as f64
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at_mut(index);
        (SliceMutProducer { slice: left }, SliceMutProducer { slice: right })
    }

    unsafe fn produce(&mut self, _: &()) -> &'data mut T {
        let slice = mem::replace(&mut self.slice, &mut []); // FIXME rust-lang/rust#10520
        let (head, tail) = slice.split_first_mut().unwrap();
        self.slice = tail;
        head
    }
}
