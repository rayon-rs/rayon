//! This module contains the parallel iterator types for slices
//! (`[T]`). You will rarely need to interact with it directly unless
//! you have need to name one of those types.

use iter::*;
use iter::internal::*;
use std::cmp;

/// Parallel extensions for slices.
///
/// Implementing this trait is not permitted outside of `rayon`.
pub trait ParallelSlice<T> {
    private_decl!{}

    /// Returns a parallel iterator over all contiguous windows of
    /// length `size`. The windows overlap.
    fn par_windows(&self, size: usize) -> Windows<T> where T: Sync;

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks do not overlap.
    fn par_chunks(&self, size: usize) -> Chunks<T> where T: Sync;

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks are mutable and do not overlap.
    fn par_chunks_mut(&mut self, size: usize) -> ChunksMut<T> where T: Send;
}

impl<T> ParallelSlice<T> for [T] {
    private_impl!{}

    fn par_windows(&self, window_size: usize) -> Windows<T>
        where T: Sync
    {
        Windows {
            window_size: window_size,
            slice: self,
        }
    }

    fn par_chunks(&self, chunk_size: usize) -> Chunks<T>
        where T: Sync
    {
        Chunks {
            chunk_size: chunk_size,
            slice: self,
        }
    }

    fn par_chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<T>
        where T: Send
    {
        ChunksMut {
            chunk_size: chunk_size,
            slice: self,
        }
    }
}

impl<'data, T: Sync + 'data> IntoParallelIterator for &'data [T] {
    type Item = &'data T;
    type Iter = Iter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { slice: self }
    }
}

impl<'data, T: Sync + 'data> IntoParallelIterator for &'data Vec<T> {
    type Item = &'data T;
    type Iter = Iter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { slice: self }
    }
}

impl<'data, T: Send + 'data> IntoParallelIterator for &'data mut [T] {
    type Item = &'data mut T;
    type Iter = IterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        IterMut { slice: self }
    }
}

impl<'data, T: Send + 'data> IntoParallelIterator for &'data mut Vec<T> {
    type Item = &'data mut T;
    type Iter = IterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        IterMut { slice: self }
    }
}


/// Parallel iterator over immutable items in a slice
pub struct Iter<'data, T: 'data + Sync> {
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Iter<'data, T> {
    type Item = &'data T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> BoundedParallelIterator for Iter<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Sync + 'data> ExactParallelIterator for Iter<'data, T> {
    fn len(&mut self) -> usize {
        self.slice.len()
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Iter<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(IterProducer { slice: self.slice })
    }
}

struct IterProducer<'data, T: 'data + Sync> {
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for IterProducer<'data, T> {
    type Item = &'data T;
    type IntoIter = ::std::slice::Iter<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at(index);
        (IterProducer { slice: left }, IterProducer { slice: right })
    }
}


/// Parallel iterator over immutable non-overlapping chunks of a slice
pub struct Chunks<'data, T: 'data + Sync> {
    chunk_size: usize,
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Chunks<'data, T> {
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> BoundedParallelIterator for Chunks<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Sync + 'data> ExactParallelIterator for Chunks<'data, T> {
    fn len(&mut self) -> usize {
        (self.slice.len() + (self.chunk_size - 1)) / self.chunk_size
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Chunks<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(ChunksProducer {
                              chunk_size: self.chunk_size,
                              slice: self.slice,
                          })
    }
}

struct ChunksProducer<'data, T: 'data + Sync> {
    chunk_size: usize,
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for ChunksProducer<'data, T> {
    type Item = &'data [T];
    type IntoIter = ::std::slice::Chunks<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks(self.chunk_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at(elem_index);
        (ChunksProducer {
             chunk_size: self.chunk_size,
             slice: left,
         },
         ChunksProducer {
             chunk_size: self.chunk_size,
             slice: right,
         })
    }
}


/// Parallel iterator over immutable overlapping windows of a slice
pub struct Windows<'data, T: 'data + Sync> {
    window_size: usize,
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Windows<'data, T> {
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> BoundedParallelIterator for Windows<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Sync + 'data> ExactParallelIterator for Windows<'data, T> {
    fn len(&mut self) -> usize {
        assert!(self.window_size >= 1);
        self.slice.len().saturating_sub(self.window_size - 1)
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Windows<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(WindowsProducer {
                              window_size: self.window_size,
                              slice: self.slice,
                          })
    }
}

struct WindowsProducer<'data, T: 'data + Sync> {
    window_size: usize,
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for WindowsProducer<'data, T> {
    type Item = &'data [T];
    type IntoIter = ::std::slice::Windows<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.windows(self.window_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let left_index = cmp::min(self.slice.len(), index + (self.window_size - 1));
        let left = &self.slice[..left_index];
        let right = &self.slice[index..];
        (WindowsProducer {
             window_size: self.window_size,
             slice: left,
         },
         WindowsProducer {
             window_size: self.window_size,
             slice: right,
         })
    }
}


/// Parallel iterator over mutable items in a slice
pub struct IterMut<'data, T: 'data + Send> {
    slice: &'data mut [T],
}

impl<'data, T: Send + 'data> ParallelIterator for IterMut<'data, T> {
    type Item = &'data mut T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Send + 'data> BoundedParallelIterator for IterMut<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Send + 'data> ExactParallelIterator for IterMut<'data, T> {
    fn len(&mut self) -> usize {
        self.slice.len()
    }
}

impl<'data, T: Send + 'data> IndexedParallelIterator for IterMut<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(IterMutProducer { slice: self.slice })
    }
}

struct IterMutProducer<'data, T: 'data + Send> {
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for IterMutProducer<'data, T> {
    type Item = &'data mut T;
    type IntoIter = ::std::slice::IterMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at_mut(index);
        (IterMutProducer { slice: left }, IterMutProducer { slice: right })
    }
}


/// Parallel iterator over mutable non-overlapping chunks of a slice
pub struct ChunksMut<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T],
}

impl<'data, T: Send + 'data> ParallelIterator for ChunksMut<'data, T> {
    type Item = &'data mut [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Send + 'data> BoundedParallelIterator for ChunksMut<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Send + 'data> ExactParallelIterator for ChunksMut<'data, T> {
    fn len(&mut self) -> usize {
        (self.slice.len() + (self.chunk_size - 1)) / self.chunk_size
    }
}

impl<'data, T: Send + 'data> IndexedParallelIterator for ChunksMut<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(ChunksMutProducer {
                              chunk_size: self.chunk_size,
                              slice: self.slice,
                          })
    }
}

struct ChunksMutProducer<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for ChunksMutProducer<'data, T> {
    type Item = &'data mut [T];
    type IntoIter = ::std::slice::ChunksMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks_mut(self.chunk_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at_mut(elem_index);
        (ChunksMutProducer {
             chunk_size: self.chunk_size,
             slice: left,
         },
         ChunksMutProducer {
             chunk_size: self.chunk_size,
             slice: right,
         })
    }
}
