//! This module contains the parallel iterator type for transposing a
//! HashMap of Vecs into a Vec of HashMaps.  Use HashMapVecTranspose::new
//! to operate the iterator.

use crate::iter::plumbing::*;
use crate::iter::*;
use crate::vec::{DrainProducer, SliceDrain};
use std::iter;

/// Parallel transposing iterator
#[derive(Debug, Clone)]
pub struct Transpose<P> {
    vec: Vec<P>,
    len: usize,
}

/// An iterator which yields the transposed items.
#[derive(Debug, Clone)]
pub struct TransposeIterator<T> {/* ??? */}

pub fn transpose<T, P, I>(iter: I, len: usize) -> Transpose<P>
where
    I: IntoIterator<Item = P>,
    P: IntoParallelIterator<Iter: IndexedParallelIterator<Item = T>>,
{
    let vec = iter.into_iter().collect();
    Transpose { vec, len }
}

impl<T, P> ParallelIterator for Transpose<P>
where
    P: Send,
    P: IntoParallelIterator<Iter: IndexedParallelIterator<Item = T>>,
{
    type Item = TransposeIterator<T>;

    fn drive_unindexed<C: UnindexedConsumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len)
    }
}

impl<T, P> IndexedParallelIterator for Transpose<P>
where
    P: Send,
    P: IntoParallelIterator<Iter: IndexedParallelIterator<Item = T>>,
{
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.len
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(mut self, callback: CB) -> CB::Output {
        // Create the producer as the exclusive "owner" of the slice.
        let producer = TransposeProducer::from_transpose(&mut self);

        // The producer will move or drop each item from the drained range.
        callback.callback(producer)
    }
}

// ////////////////////////////////////////////////////////////////////////
struct TransposeProducer<'data, T: Send> {
    map: Vec<DrainProducer<'data, T>>,
    len: usize,
}

impl<'data, T: Send> TransposeProducer<'data, T> {
    fn new(map: Vec<DrainProducer<'data, T>>, len: usize) -> Self {
        Self { map, len }
    }

    fn from_transpose<P>(transpose: &'data mut Transpose<P>) -> Self
    where
        P: Send,
        P: IntoParallelIterator<Iter: IndexedParallelIterator<Item = T>>,
    {
        let len = transpose.len;
        let map = transpose
            .vec
            .iter_mut()
            .map(|vec| {
                assert_eq!(vec.len(), len);
                unsafe {
                    vec.set_len(0);
                    DrainProducer::from_vec(vec, len)
                }
            })
            .collect();
        Self::new(map, len)
    }
}

impl<'data, T: 'data + Send> Producer for TransposeProducer<'data, T> {
    type Item = TransposeIterator<T>;
    type IntoIter = TransposeSliceDrain<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len;
        let map = self.map.into_iter().map(IntoIterator::into_iter).collect();
        TransposeSliceDrain { map, len }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        // TODO: figure out if there is a way to reuse the allocation of self
        let (left, right) = self.map.into_iter().map(Producer::split_at).collect();
        (Self::new(left, index), Self::new(right, self.len - index))
    }
}

// ////////////////////////////////////////////////////////////////////////

// like std::vec::Drain, without updating a source Vec
struct TransposeSliceDrain<'data, T> {
    map: Vec<SliceDrain<'data, T>>,
    len: usize,
}

impl<'data, T: 'data> Iterator for TransposeSliceDrain<'data, T> {
    type Item = TransposeIterator<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.map.iter_mut().map(Iterator::next)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }

    fn count(self) -> usize {
        self.len
    }
}

impl<'data, T: 'data> DoubleEndedIterator for TransposeSliceDrain<'data, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.map.iter_mut().map(Iterator::next_back)
    }
}

impl<'data, T: 'data> ExactSizeIterator for TransposeSliceDrain<'data, T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'data, T: 'data> iter::FusedIterator for TransposeSliceDrain<'data, T> {}
