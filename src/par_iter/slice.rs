use super::*;
use super::len::ParallelLen;
use super::state::*;

pub struct SliceIter<'data, T: 'data + Sync> {
    slice: &'data [T]
}

impl<'data, T: Sync> IntoParallelIterator for &'data [T] {
    type Item = &'data T;
    type Iter = SliceIter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIter { slice: self }
    }
}

impl<'data, T: Sync + 'data> IntoParallelRefIterator<'data> for [T] {
    type Item = T;
    type Iter = SliceIter<'data, T>;

    fn par_iter(&'data self) -> Self::Iter {
        self.into_par_iter()
    }
}

impl<'data, T: Sync> ParallelIterator for SliceIter<'data, T> {
    type Item = &'data T;
    type Shared = ();
    type State = Self;

    fn drive<C: Consumer<Item=Self::Item>>(self, consumer: C, shared: C::Shared) -> C::Result {
        bridge(self, consumer, &shared)
    }

    fn state(self) -> (Self::Shared, Self::State) {
        ((), self)
    }
}

unsafe impl<'data, T: Sync> BoundedParallelIterator for SliceIter<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }
}

unsafe impl<'data, T: Sync> ExactParallelIterator for SliceIter<'data, T> {
    fn len(&mut self) -> usize {
        self.slice.len() as usize
    }
}

impl<'data, T: Sync> PullParallelIterator for SliceIter<'data, T> {
    type Producer = SliceProducer<'data, T>;

    fn into_producer(self) -> (Self::Producer, ()) {
        (SliceProducer { slice: self.slice }, ())
    }
}

unsafe impl<'data, T: Sync> ParallelIteratorState for SliceIter<'data, T> {
    type Item = &'data T;
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

    fn next(&mut self, _shared: &Self::Shared) -> Option<&'data T> {
        self.slice.split_first()
                  .map(|(head, tail)| {
                      self.slice = tail;
                      head
                  })
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct SliceProducer<'data, T: 'data + Sync> {
    slice: &'data [T]
}

impl<'data, T: 'data + Sync> Producer for SliceProducer<'data, T>
{
    type Item = &'data T;
    type Shared = ();

    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64 {
        len as f64
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at(index);
        (SliceProducer { slice: left }, SliceProducer { slice: right })
    }

    unsafe fn produce(&mut self, _: &()) -> &'data T {
        let (head, tail) = self.slice.split_first().unwrap();
        self.slice = tail;
        head
    }
}
