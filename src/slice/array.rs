use crate::iter::plumbing::*;
use crate::iter::*;

use super::{Iter, IterMut};

/// Parallel iterator over immutable non-overlapping chunks of a slice
#[derive(Debug)]
pub struct ArrayChunks<'data, T: Sync, const N: usize> {
    iter: Iter<'data, [T; N]>,
    rem: &'data [T],
}

impl<'data, T: Sync, const N: usize> ArrayChunks<'data, T, N> {
    pub(super) fn new(slice: &'data [T]) -> Self {
        assert_ne!(N, 0);
        let len = slice.len() / N;
        let (fst, snd) = slice.split_at(len * N);
        // SAFETY: We cast a slice of `len * N` elements into
        // a slice of `len` many `N` elements chunks.
        let array_slice: &'data [[T; N]] = unsafe {
            let ptr = fst.as_ptr() as *const [T; N];
            ::std::slice::from_raw_parts(ptr, len)
        };
        Self {
            iter: array_slice.par_iter(),
            rem: snd,
        }
    }

    /// Return the remainder of the original slice that is not going to be
    /// returned by the iterator. The returned slice has at most `N-1`
    /// elements.
    pub fn remainder(&self) -> &'data [T] {
        self.rem
    }
}

impl<'data, T: Sync, const N: usize> Clone for ArrayChunks<'data, T, N> {
    fn clone(&self) -> Self {
        ArrayChunks {
            iter: self.iter.clone(),
            rem: self.rem,
        }
    }
}

impl<'data, T: Sync + 'data, const N: usize> ParallelIterator for ArrayChunks<'data, T, N> {
    type Item = &'data [T; N];

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

impl<'data, T: Sync + 'data, const N: usize> IndexedParallelIterator for ArrayChunks<'data, T, N> {
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.iter.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        self.iter.with_producer(callback)
    }
}

/// Parallel iterator over immutable non-overlapping chunks of a slice
#[derive(Debug)]
pub struct ArrayChunksMut<'data, T: Send, const N: usize> {
    iter: IterMut<'data, [T; N]>,
    rem: &'data mut [T],
}

impl<'data, T: Send, const N: usize> ArrayChunksMut<'data, T, N> {
    pub(super) fn new(slice: &'data mut [T]) -> Self {
        assert_ne!(N, 0);
        let len = slice.len() / N;
        let (fst, snd) = slice.split_at_mut(len * N);
        // SAFETY: We cast a slice of `len * N` elements into
        // a slice of `len` many `N` elements chunks.
        let array_slice: &'data mut [[T; N]] = unsafe {
            let ptr = fst.as_mut_ptr() as *mut [T; N];
            ::std::slice::from_raw_parts_mut(ptr, len)
        };
        Self {
            iter: array_slice.par_iter_mut(),
            rem: snd,
        }
    }

    /// Return the remainder of the original slice that is not going to be
    /// returned by the iterator. The returned slice has at most `N-1`
    /// elements.
    ///
    /// Note that this has to consume `self` to return the original lifetime of
    /// the data, which prevents this from actually being used as a parallel
    /// iterator since that also consumes. This method is provided for parity
    /// with `std::iter::ArrayChunksMut`, but consider calling `remainder()` or
    /// `take_remainder()` as alternatives.
    pub fn into_remainder(self) -> &'data mut [T] {
        self.rem
    }

    /// Return the remainder of the original slice that is not going to be
    /// returned by the iterator. The returned slice has at most `N-1`
    /// elements.
    ///
    /// Consider `take_remainder()` if you need access to the data with its
    /// original lifetime, rather than borrowing through `&mut self` here.
    pub fn remainder(&mut self) -> &mut [T] {
        self.rem
    }

    /// Return the remainder of the original slice that is not going to be
    /// returned by the iterator. The returned slice has at most `N-1`
    /// elements. Subsequent calls will return an empty slice.
    pub fn take_remainder(&mut self) -> &'data mut [T] {
        std::mem::replace(&mut self.rem, &mut [])
    }
}

impl<'data, T: Send + 'data, const N: usize> ParallelIterator for ArrayChunksMut<'data, T, N> {
    type Item = &'data mut [T; N];

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

impl<'data, T: Send + 'data, const N: usize> IndexedParallelIterator
    for ArrayChunksMut<'data, T, N>
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.iter.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        self.iter.with_producer(callback)
    }
}
