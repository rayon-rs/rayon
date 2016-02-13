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

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<T: Send> BoundedParallelIterator for VecIter<T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<T: Send> ExactParallelIterator for VecIter<T> {
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
        callback.callback(producer)
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

impl<'data, T: 'data + Send> Producer for VecProducer<'data, T> {
    fn cost(&mut self, len: usize) -> f64 {
        len as f64
    }

    fn split_at(mut self, index: usize) -> (Self, Self) {
        // replace the slice so we don't drop it twice
        let slice = std::mem::replace(&mut self.slice, &mut []);
        let (left, right) = slice.split_at_mut(index);
        (VecProducer { slice: left }, VecProducer { slice: right })
    }
}

impl<'data, T: 'data + Send> IntoIterator for VecProducer<'data, T> {
    type Item = T;
    type IntoIter = SliceDrain<'data, T>;

    fn into_iter(mut self) -> Self::IntoIter {
        // replace the slice so we don't drop it twice
        let slice = std::mem::replace(&mut self.slice, &mut []);
        SliceDrain { iter: slice.iter_mut() }
    }
}

impl<'data, T: 'data + Send> Drop for VecProducer<'data, T> {
    fn drop(&mut self) {
        SliceDrain { iter: self.slice.iter_mut() };
    }
}

///////////////////////////////////////////////////////////////////////////

// like std::vec::Drain, without updating a source Vec
pub struct SliceDrain<'data, T: 'data> {
    iter: std::slice::IterMut<'data, T>
}

impl<'data, T: 'data> Iterator for SliceDrain<'data, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.iter.next()
            .map(|ptr| unsafe { std::ptr::read(ptr) })
    }
}

impl<'data, T: 'data> Drop for SliceDrain<'data, T> {
    fn drop(&mut self) {
        for ptr in &mut self.iter {
            // use drop_in_place once stable
            unsafe { std::ptr::read(ptr); }
        }
    }
}
