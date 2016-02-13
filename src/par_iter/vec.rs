use super::*;
use super::internal::*;
use std;

pub struct VecIter<T: Send> {
    vec: Vec<T>,
    consumed: bool,
}

impl<T: Send> IntoParallelIterator for Vec<T> {
    type Item = T;
    type Iter = VecIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        VecIter { vec: self, consumed: false }
    }
}

impl<T: Send> ParallelIterator for VecIter<T> {
    type Item = T;

    fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result {
        bridge(self, consumer, &shared)
    }
}

unsafe impl<T: Send> BoundedParallelIterator for VecIter<T> {
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

unsafe impl<T: Send> ExactParallelIterator for VecIter<T> {
    fn len(&mut self) -> usize {
        self.vec.len()
    }
}

impl<T: Send> IndexedParallelIterator for VecIter<T> {
    fn with_producer<CB>(mut self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        // The producer will move or drop each item from the slice.
        let producer = VecProducer { slice: &mut self.vec };
        self.consumed = true;
        callback.callback(producer, &())
    }
}

impl<T: Send> Drop for VecIter<T> {
    fn drop(&mut self) {
        if self.consumed {
            // only need vec to free its buffer now
            unsafe { self.vec.set_len(0) };
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct VecProducer<'data, T: 'data + Send> {
    slice: &'data mut [T]
}

impl<'p, 'data, T: 'data + Send> Producer<'p> for VecProducer<'data, T> {
    type Item = T;
    type Shared = ();

    fn cost(&mut self, _: &Self::Shared, len: usize) -> f64 {
        len as f64
    }

    unsafe fn split_at(mut self, index: usize) -> (Self, Self) {
        // replace the slice so we don't drop it twice
        let slice = std::mem::replace(&mut self.slice, &mut []);
        let (left, right) = slice.split_at_mut(index);
        (VecProducer { slice: left }, VecProducer { slice: right })
    }

    unsafe fn produce(&mut self, _: &()) -> T {
        let slice = std::mem::replace(&mut self.slice, &mut []); // FIXME rust-lang/rust#10520
        let (head, tail) = slice.split_first_mut().unwrap();
        self.slice = tail;
        std::ptr::read(head)
    }
}

impl<'data, T: 'data + Send> Drop for VecProducer<'data, T> {
    fn drop(&mut self) {
        for ptr in self.slice.iter_mut() {
            // use drop_in_place once stable
            unsafe { std::ptr::read(ptr); }
        }
    }
}
