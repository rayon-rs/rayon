use super::plumbing::*;
use super::{IndexedParallelIterator, ParallelIterator};

/// `CartesianProduct` is an iterator that iterates over the cartesian product
/// of the elements of two parallel iterators.
///
/// This struct is created by the [`cartesian_product()`] method on
/// [`IndexedParallelIterator`].
///
/// [`cartesian_product()`]: IndexedParallelIterator::cartesian_product
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct CartesianProduct<I, J> {
    i: I,
    j: J,
}

impl<I, J> CartesianProduct<I, J>
where
    I: IndexedParallelIterator,
    J: IndexedParallelIterator,
{
    /// Creates a new `CartesianProduct` iterator.
    pub(super) fn new(i: I, j: J) -> Self {
        CartesianProduct { i, j }
    }

    fn total_len(&self) -> usize {
        self.i
            .len()
            .checked_mul(self.j.len())
            .expect("cartesian_product length overflows usize")
    }
}

impl<I, J> ParallelIterator for CartesianProduct<I, J>
where
    I: IndexedParallelIterator,
    J: IndexedParallelIterator,
    I::Item: Clone + Sync,
    J::Item: Clone + Sync,
{
    type Item = (I::Item, J::Item);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        // Both inputs are indexed, so `len()` is always available (an indexed
        // iterator may still return `None` from `opt_len`). `None` only on
        // overflow.
        self.i.len().checked_mul(self.j.len())
    }
}

impl<I, J> IndexedParallelIterator for CartesianProduct<I, J>
where
    I: IndexedParallelIterator,
    J: IndexedParallelIterator,
    I::Item: Clone + Sync,
    J::Item: Clone + Sync,
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.total_len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        let len = self.total_len();
        if len == 0 {
            // One of the inputs is empty, so the product is too; avoid buffering
            // the (possibly large) other input.
            return callback.callback(CartesianProductProducer {
                i: &[],
                j: &[],
                range: 0..0,
            });
        }
        // Buffer both inputs so their elements can be paired by index in
        // parallel. This is an `O(i.len() + j.len())` allocation, and drives both
        // inputs to completion before any pair is produced.
        let i: Vec<I::Item> = self.i.collect();
        let j: Vec<J::Item> = self.j.collect();
        callback.callback(CartesianProductProducer {
            i: &i,
            j: &j,
            range: 0..len,
        })
    }
}

/// Producer for `CartesianProduct`, indexing flat positions `k` in `range` as
/// `(i[k / j.len()], j[k % j.len()])` (row-major order).
struct CartesianProductProducer<'a, A, B> {
    i: &'a [A],
    j: &'a [B],
    range: std::ops::Range<usize>,
}

impl<'a, A, B> Producer for CartesianProductProducer<'a, A, B>
where
    A: Clone + Send + Sync,
    B: Clone + Send + Sync,
{
    type Item = (A, B);
    type IntoIter = CartesianProductSeq<'a, A, B>;

    fn into_iter(self) -> Self::IntoIter {
        CartesianProductSeq {
            i: self.i,
            j: self.j,
            range: self.range,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        // `index` is relative to the start of this producer's range.
        let mid = self.range.start + index;
        debug_assert!(mid <= self.range.end);
        (
            CartesianProductProducer {
                i: self.i,
                j: self.j,
                range: self.range.start..mid,
            },
            CartesianProductProducer {
                i: self.i,
                j: self.j,
                range: mid..self.range.end,
            },
        )
    }
}

/// Sequential iterator backing `CartesianProductProducer`.
struct CartesianProductSeq<'a, A, B> {
    i: &'a [A],
    j: &'a [B],
    range: std::ops::Range<usize>,
}

impl<'a, A, B> CartesianProductSeq<'a, A, B>
where
    A: Clone,
    B: Clone,
{
    fn get(&self, k: usize) -> (A, B) {
        // `range` is only non-empty when `j` is non-empty, so `j.len() != 0`.
        let n = self.j.len();
        (self.i[k / n].clone(), self.j[k % n].clone())
    }
}

impl<'a, A, B> Iterator for CartesianProductSeq<'a, A, B>
where
    A: Clone,
    B: Clone,
{
    type Item = (A, B);

    fn next(&mut self) -> Option<Self::Item> {
        self.range.next().map(|k| self.get(k))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl<'a, A, B> DoubleEndedIterator for CartesianProductSeq<'a, A, B>
where
    A: Clone,
    B: Clone,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.range.next_back().map(|k| self.get(k))
    }
}

impl<'a, A, B> ExactSizeIterator for CartesianProductSeq<'a, A, B>
where
    A: Clone,
    B: Clone,
{
    fn len(&self) -> usize {
        self.range.len()
    }
}
