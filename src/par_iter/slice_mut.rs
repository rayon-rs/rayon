use super::*;
use super::internal::*;

pub struct SliceIterMut<'data, T: 'data + Send> {
    slice: &'data mut [T]
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

impl<'data, T: Send + 'data> ToParallelChunksMut<'data> for [T] {
    type Item = T;
    type Iter = ChunksMutIter<'data, T>;

    fn par_chunks_mut(&'data mut self, chunk_size: usize) -> Self::Iter {
        ChunksMutIter { chunk_size: chunk_size, slice: self }
    }
}

impl<'data, T: Send + 'data> ParallelIterator for SliceIterMut<'data, T> {
    type Item = &'data mut T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
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
        callback.callback(SliceChunksMutProducer { chunk_size: self.chunk_size, slice: self.slice })
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct SliceMutProducer<'data, T: 'data + Send> {
    slice: &'data mut [T]
}

impl<'data, T: 'data + Send> Producer for SliceMutProducer<'data, T>
{
    fn cost(&mut self, len: usize) -> f64 {
        len as f64
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at_mut(index);
        (SliceMutProducer { slice: left }, SliceMutProducer { slice: right })
    }
}

impl<'data, T: 'data + Send> IntoIterator for SliceMutProducer<'data, T> {
    type Item = &'data mut T;
    type IntoIter = ::std::slice::IterMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }
}

pub struct SliceChunksMutProducer<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T]
}

impl<'data, T: 'data + Send> Producer for SliceChunksMutProducer<'data, T> {
    fn cost(&mut self, len: usize) -> f64 {
        len as f64
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at_mut(elem_index);
        (SliceChunksMutProducer { chunk_size: self.chunk_size, slice: left },
         SliceChunksMutProducer { chunk_size: self.chunk_size, slice: right })
    }
}

impl<'data, T: 'data + Send> IntoIterator for SliceChunksMutProducer<'data, T> {
    type Item = &'data mut [T];
    type IntoIter = ::std::slice::ChunksMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks_mut(self.chunk_size)
    }
}
