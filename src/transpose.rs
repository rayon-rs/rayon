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
        let producers = self
            .vec
            .into_iter()
            .map(|iter| {
                // I don't know how to get the Producer from the iter here.
                iter.into_par_iter().with_producer(callback)
            })
            .collect();
        let producer = TransposeProducer {
            producers,
            len: self.len,
        };

        // The producer will move or drop each item from the drained range.
        callback.callback(producer)
    }
}

// ////////////////////////////////////////////////////////////////////////
struct TransposeProducer<Pr> {
    producers: Vec<Pr>,
    len: usize,
}

impl<Pr: Send> TransposeProducer<Pr> {
    fn new(producers: Vec<Pr>, len: usize) -> Self {
        Self { producers, len }
    }

    fn from_transpose<P>(transpose: &mut Transpose<P>) -> Self
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

impl<Pr, T> Producer for TransposeProducer<Pr>
where
    Pr: Producer<Item = T>,
{
    type Item = T;
    type IntoIter = TransposeSliceDrain<Pr::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len;
        let map = self
            .producers
            .into_iter()
            .map(Producer::into_iter)
            .collect();
        TransposeSliceDrain { map, len }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        // TODO: figure out if there is a way to reuse the allocation of self
        let (left, right) = self
            .producers
            .into_iter()
            .map(|producer| producer.split_at(index))
            .collect();
        (Self::new(left, index), Self::new(right, self.len - index))
    }
}

// ////////////////////////////////////////////////////////////////////////

// like std::vec::Drain, without updating a source Vec
struct TransposeSliceDrain<I> {
    map: Vec<I>,
    len: usize,
}

impl<I> Iterator for TransposeSliceDrain<I> {
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

impl<I> DoubleEndedIterator for TransposeSliceDrain<I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.map.iter_mut().map(Iterator::next_back)
    }
}

impl<I> ExactSizeIterator for TransposeSliceDrain<I> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<I> iter::FusedIterator for TransposeSliceDrain<I> {}
