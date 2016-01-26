use super::*;
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

    fn drive_stateless<'c, C: StatelessConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result {
        bridge(self, consumer, &shared)
    }
}

unsafe impl<'data, T: Send> BoundedParallelIterator for SliceIterMut<'data, T> {
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

unsafe impl<'data, T: Send> ExactParallelIterator for SliceIterMut<'data, T> {
    fn len(&mut self) -> usize {
        self.slice.len() as usize
    }
}

impl<'data, T: Send> IndexedParallelIterator for SliceIterMut<'data, T> {
    type Producer = SliceMutProducer<'data, T>;

    fn into_producer(self) -> (Self::Producer, ()) {
        (SliceMutProducer { slice: self.slice }, ())
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

    fn cost(&mut self, _: &Self::Shared, len: usize) -> f64 {
        len as f64
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
