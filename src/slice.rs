//! This module contains the parallel iterator types for slices
//! (`[T]`). You will rarely need to interact with it directly unless
//! you have need to name one of those types.

use iter::*;
use iter::internal::*;

/// Parallel extensions for slices.
///
/// Implementing this trait is not permitted outside of `rayon`.
pub trait ParallelSlice<T> {
    private_decl!{}

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks do not overlap.
    fn par_chunks(&self, size: usize) -> ChunksIter<T> where T: Sync;

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks are mutable and do not overlap.
    fn par_chunks_mut(&mut self, size: usize) -> ChunksMutIter<T> where T: Send;
}

pub struct SliceIter<'data, T: 'data + Sync> {
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> IntoParallelIterator for &'data [T] {
    type Item = &'data T;
    type Iter = SliceIter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIter { slice: self }
    }
}

impl<'data, T: Sync + 'data> IntoParallelIterator for &'data Vec<T> {
    type Item = &'data T;
    type Iter = SliceIter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIter { slice: self }
    }
}

impl<T> ParallelSlice<T> for [T] {
    private_impl!{}

    fn par_chunks(&self, chunk_size: usize) -> ChunksIter<T> where T: Sync {
        ChunksIter {
            chunk_size: chunk_size,
            slice: self,
        }
    }

    fn par_chunks_mut(&mut self, chunk_size: usize) -> ChunksMutIter<T> where T: Send {
        ChunksMutIter {
            chunk_size: chunk_size,
            slice: self,
        }
    }
}

impl<'data, T: Sync + 'data> ParallelIterator for SliceIter<'data, T> {
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

impl<'data, T: Sync + 'data> BoundedParallelIterator for SliceIter<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Sync + 'data> ExactParallelIterator for SliceIter<'data, T> {
    fn len(&mut self) -> usize {
        self.slice.len()
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for SliceIter<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(SliceProducer { slice: self.slice })
    }
}

pub struct ChunksIter<'data, T: 'data + Sync> {
    chunk_size: usize,
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for ChunksIter<'data, T> {
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

impl<'data, T: Sync + 'data> BoundedParallelIterator for ChunksIter<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Sync + 'data> ExactParallelIterator for ChunksIter<'data, T> {
    fn len(&mut self) -> usize {
        (self.slice.len() + (self.chunk_size - 1)) / self.chunk_size
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for ChunksIter<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(SliceChunksProducer {
            chunk_size: self.chunk_size,
            slice: self.slice,
        })
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct SliceProducer<'data, T: 'data + Sync> {
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for SliceProducer<'data, T> {
    type Item = &'data T;
    type IntoIter = ::std::slice::Iter<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at(index);
        (SliceProducer { slice: left }, SliceProducer { slice: right })
    }
}

struct SliceChunksProducer<'data, T: 'data + Sync> {
    chunk_size: usize,
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for SliceChunksProducer<'data, T> {
    type Item = &'data [T];
    type IntoIter = ::std::slice::Chunks<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks(self.chunk_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at(elem_index);
        (SliceChunksProducer {
             chunk_size: self.chunk_size,
             slice: left,
         },
         SliceChunksProducer {
             chunk_size: self.chunk_size,
             slice: right,
         })
    }
}

pub struct SliceIterMut<'data, T: 'data + Send> {
    slice: &'data mut [T],
}

impl<'data, T: Send + 'data> IntoParallelIterator for &'data mut [T] {
    type Item = &'data mut T;
    type Iter = SliceIterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIterMut { slice: self }
    }
}

impl<'data, T: Send + 'data> IntoParallelIterator for &'data mut Vec<T> {
    type Item = &'data mut T;
    type Iter = SliceIterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIterMut { slice: self }
    }
}

impl<'data, T: Send + 'data> ParallelIterator for SliceIterMut<'data, T> {
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

impl<'data, T: Send + 'data> BoundedParallelIterator for SliceIterMut<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Send + 'data> ExactParallelIterator for SliceIterMut<'data, T> {
    fn len(&mut self) -> usize {
        self.slice.len()
    }
}

impl<'data, T: Send + 'data> IndexedParallelIterator for SliceIterMut<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(SliceMutProducer { slice: self.slice })
    }
}

pub struct ChunksMutIter<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T],
}

impl<'data, T: Send + 'data> ParallelIterator for ChunksMutIter<'data, T> {
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

impl<'data, T: Send + 'data> BoundedParallelIterator for ChunksMutIter<'data, T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<'data, T: Send + 'data> ExactParallelIterator for ChunksMutIter<'data, T> {
    fn len(&mut self) -> usize {
        (self.slice.len() + (self.chunk_size - 1)) / self.chunk_size
    }
}

impl<'data, T: Send + 'data> IndexedParallelIterator for ChunksMutIter<'data, T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(SliceChunksMutProducer {
            chunk_size: self.chunk_size,
            slice: self.slice,
        })
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct SliceMutProducer<'data, T: 'data + Send> {
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for SliceMutProducer<'data, T> {
    type Item = &'data mut T;
    type IntoIter = ::std::slice::IterMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at_mut(index);
        (SliceMutProducer { slice: left }, SliceMutProducer { slice: right })
    }
}

struct SliceChunksMutProducer<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for SliceChunksMutProducer<'data, T> {
    type Item = &'data mut [T];
    type IntoIter = ::std::slice::ChunksMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks_mut(self.chunk_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at_mut(elem_index);
        (SliceChunksMutProducer {
             chunk_size: self.chunk_size,
             slice: left,
         },
         SliceChunksMutProducer {
             chunk_size: self.chunk_size,
             slice: right,
         })
    }
}
