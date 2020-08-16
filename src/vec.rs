//! Parallel iterator types for [vectors][std::vec] (`Vec<T>`)
//!
//! You will rarely need to interact with this module directly unless you need
//! to name one of the iterator types.
//!
//! [std::vec]: https://doc.rust-lang.org/stable/std/vec/

use crate::iter::plumbing::*;
use crate::iter::*;
use crate::math::simplify_range;
use std::iter;
use std::mem;
use std::ops::{Range, RangeBounds};
use std::ptr;
use std::slice;

/// Parallel iterator that moves out of a vector.
#[derive(Debug, Clone)]
pub struct IntoIter<T: Send> {
    vec: Vec<T>,
}

impl<T: Send> IntoParallelIterator for Vec<T> {
    type Item = T;
    type Iter = IntoIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        IntoIter { vec: self }
    }
}

impl<T: Send> ParallelIterator for IntoIter<T> {
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<T: Send> IndexedParallelIterator for IntoIter<T> {
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.vec.len()
    }

    fn with_producer<CB>(mut self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        // Drain every item, and then the vector only needs to free its buffer.
        self.vec.par_drain(..).with_producer(callback)
    }
}

impl<'data, T: Send> ParallelDrainRange<usize> for &'data mut Vec<T> {
    type Iter = Drain<'data, T>;
    type Item = T;

    fn par_drain<R: RangeBounds<usize>>(self, range: R) -> Self::Iter {
        Drain {
            orig_len: self.len(),
            range: simplify_range(range, self.len()),
            vec: self,
        }
    }
}

/// Draining parallel iterator that moves a range out of a vector, but keeps the total capacity.
#[derive(Debug)]
pub struct Drain<'data, T: Send> {
    vec: &'data mut Vec<T>,
    range: Range<usize>,
    orig_len: usize,
}

impl<'data, T: Send> ParallelIterator for Drain<'data, T> {
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Send> IndexedParallelIterator for Drain<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.range.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        unsafe {
            // Make the vector forget about the drained items, and temporarily the tail too.
            let start = self.range.start;
            self.vec.set_len(start);

            // Get a correct borrow lifetime, then extend it to the original length.
            let mut slice = &mut self.vec[start..];
            slice = slice::from_raw_parts_mut(slice.as_mut_ptr(), self.range.len());

            // The producer will move or drop each item from the drained range.
            callback.callback(DrainProducer::new(slice))
        }
    }
}

impl<'data, T: Send> Drop for Drain<'data, T> {
    fn drop(&mut self) {
        if self.range.len() > 0 {
            let Range { start, end } = self.range;
            if self.vec.len() != start {
                // We must not have produced, so just call a normal drain to remove the items.
                assert_eq!(self.vec.len(), self.orig_len);
                self.vec.drain(start..end);
            } else if end < self.orig_len {
                // The producer was responsible for consuming the drained items.
                // Move the tail items to their new place, then set the length to include them.
                unsafe {
                    let ptr = self.vec.as_mut_ptr().add(start);
                    let tail_ptr = self.vec.as_ptr().add(end);
                    let tail_len = self.orig_len - end;
                    ptr::copy(tail_ptr, ptr, tail_len);
                    self.vec.set_len(start + tail_len);
                }
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

pub(crate) struct DrainProducer<'data, T: Send> {
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> DrainProducer<'data, T> {
    /// Creates a draining producer, which *moves* items from the slice.
    ///
    /// Unsafe bacause `!Copy` data must not be read after the borrow is released.
    pub(crate) unsafe fn new(slice: &'data mut [T]) -> Self {
        DrainProducer { slice }
    }
}

impl<'data, T: 'data + Send> Producer for DrainProducer<'data, T> {
    type Item = T;
    type IntoIter = SliceDrain<'data, T>;

    fn into_iter(mut self) -> Self::IntoIter {
        // replace the slice so we don't drop it twice
        let slice = mem::replace(&mut self.slice, &mut []);
        SliceDrain {
            iter: slice.iter_mut(),
        }
    }

    fn split_at(mut self, index: usize) -> (Self, Self) {
        // replace the slice so we don't drop it twice
        let slice = mem::replace(&mut self.slice, &mut []);
        let (left, right) = slice.split_at_mut(index);
        unsafe { (DrainProducer::new(left), DrainProducer::new(right)) }
    }
}

impl<'data, T: 'data + Send> Drop for DrainProducer<'data, T> {
    fn drop(&mut self) {
        // use `Drop for [T]`
        unsafe { ptr::drop_in_place(self.slice) };
    }
}

/// ////////////////////////////////////////////////////////////////////////

// like std::vec::Drain, without updating a source Vec
pub(crate) struct SliceDrain<'data, T> {
    iter: slice::IterMut<'data, T>,
}

impl<'data, T: 'data> Iterator for SliceDrain<'data, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let ptr = self.iter.next()?;
        Some(unsafe { ptr::read(ptr) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn count(self) -> usize {
        self.iter.len()
    }
}

impl<'data, T: 'data> DoubleEndedIterator for SliceDrain<'data, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let ptr = self.iter.next_back()?;
        Some(unsafe { ptr::read(ptr) })
    }
}

impl<'data, T: 'data> ExactSizeIterator for SliceDrain<'data, T> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'data, T: 'data> iter::FusedIterator for SliceDrain<'data, T> {}

impl<'data, T: 'data> Drop for SliceDrain<'data, T> {
    fn drop(&mut self) {
        // extract the iterator so we can use `Drop for [T]`
        let iter = mem::replace(&mut self.iter, [].iter_mut());
        unsafe { ptr::drop_in_place(iter.into_slice()) };
    }
}
