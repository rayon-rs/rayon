use crate::iter::plumbing::*;
use crate::iter::*;

use super::Iter;

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
