//! Parallel iterator types for [vectors][std::vec] (`Vec<T>`)
//!
//! You will rarely need to interact with this module directly unless you need
//! to name one of the iterator types.
//!
//! [std::vec]: https://doc.rust-lang.org/stable/std/vec/

use crate::iter::plumbing::*;
use crate::iter::*;

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
        // The producer will move or drop each item from its slice, effectively taking ownership of
        // them.  When we're done, the vector only needs to free its buffer.
        unsafe {
            // Make the vector forget about the actual items.
            let len = self.vec.len();
            self.vec.set_len(0);

            // Get a correct borrow, then extend it to the original length.
            let mut slice = self.vec.as_mut_slice();
            slice = std::slice::from_raw_parts_mut(slice.as_mut_ptr(), len);

            callback.callback(VecProducer { slice })
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct VecProducer<'data, T: Send> {
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for VecProducer<'data, T> {
    type Item = T;
    type IntoIter = SliceDrain<'data, T>;

    fn into_iter(mut self) -> Self::IntoIter {
        // replace the slice so we don't drop it twice
        let slice = std::mem::replace(&mut self.slice, &mut []);
        SliceDrain {
            iter: slice.iter_mut(),
        }
    }

    fn split_at(mut self, index: usize) -> (Self, Self) {
        // replace the slice so we don't drop it twice
        let slice = std::mem::replace(&mut self.slice, &mut []);
        let (left, right) = slice.split_at_mut(index);
        (VecProducer { slice: left }, VecProducer { slice: right })
    }
}

impl<'data, T: 'data + Send> Drop for VecProducer<'data, T> {
    fn drop(&mut self) {
        SliceDrain {
            iter: self.slice.iter_mut(),
        };
    }
}

/// ////////////////////////////////////////////////////////////////////////

// like std::vec::Drain, without updating a source Vec
struct SliceDrain<'data, T> {
    iter: std::slice::IterMut<'data, T>,
}

impl<'data, T: 'data> Iterator for SliceDrain<'data, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let ptr = self.iter.next()?;
        Some(unsafe { std::ptr::read(ptr) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'data, T: 'data> DoubleEndedIterator for SliceDrain<'data, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let ptr = self.iter.next_back()?;
        Some(unsafe { std::ptr::read(ptr) })
    }
}

impl<'data, T: 'data> ExactSizeIterator for SliceDrain<'data, T> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'data, T: 'data> Drop for SliceDrain<'data, T> {
    fn drop(&mut self) {
        for ptr in &mut self.iter {
            unsafe {
                std::ptr::drop_in_place(ptr);
            }
        }
    }
}
