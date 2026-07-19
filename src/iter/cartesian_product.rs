use super::plumbing::*;
use super::{IndexedParallelIterator, ParallelIterator};
use std::sync::Arc;

/// `CartesianProduct` is an iterator that pairs every element of one parallel
/// iterator with every element of another, in row-major order.
///
/// This struct is created by the [`cartesian_product()`] method on
/// [`ParallelIterator`].
///
/// [`cartesian_product()`]: ParallelIterator::cartesian_product
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct CartesianProduct<I, J> {
    i: I,
    j: J,
}

impl<I, J> CartesianProduct<I, J> {
    /// Creates a new `CartesianProduct` iterator.
    pub(super) fn new(i: I, j: J) -> Self {
        CartesianProduct { i, j }
    }
}

impl<I, J> ParallelIterator for CartesianProduct<I, J>
where
    I: ParallelIterator,
    J: ParallelIterator,
    I::Item: Clone + Sync,
    J::Item: Clone + Sync,
{
    type Item = (I::Item, J::Item);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        // Nothing is wanted (e.g. `take_any(0)`); don't drive either input.
        if consumer.full() {
            return consumer.into_folder().complete();
        }
        // If the outer is known-empty, the product is empty; don't buffer the
        // (possibly large) inner.
        if self.i.opt_len() == Some(0) {
            return consumer.into_folder().complete();
        }
        // Buffer ONLY the inner side. The outer streams through its own drive,
        // so peak auxiliary memory is `O(inner.len())`, not `O(outer + inner)`.
        let inner: Arc<[J::Item]> = self.j.collect::<Vec<_>>().into();
        if inner.is_empty() {
            return consumer.into_folder().complete();
        }
        // Wrap the consumer so each outer element `a` expands to the whole inner
        // buffer. A consumer split at outer index `k` maps to an output split at
        // `k * inner.len()`, so exact (preallocating) collection works while the
        // outer stays streamed.
        self.i.drive_unindexed(ExpandConsumer {
            base: consumer,
            inner,
        })
    }

    fn opt_len(&self) -> Option<usize> {
        // No buffering needed to know the length: the product of the two lengths,
        // when both are known. `None` on overflow or when either is unknown.
        self.i.opt_len()?.checked_mul(self.j.opt_len()?)
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
        self.i
            .len()
            .checked_mul(self.j.len())
            .expect("cartesian_product length overflows usize")
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        let len = self.len();
        if len == 0 {
            // One of the inputs is empty, so the product is too; avoid buffering
            // the (possibly large) other input.
            return callback.callback(CartesianProductProducer {
                i: &[],
                j: &[],
                range: 0..0,
            });
        }
        // The indexed producer must serve arbitrary flat splits, which can cut
        // through the middle of a row. That needs random access to both inputs,
        // so this path (used by `collect_into_vec`) buffers both.
        let i: Vec<I::Item> = self.i.collect();
        let j: Vec<J::Item> = self.j.collect();
        callback.callback(CartesianProductProducer {
            i: &i,
            j: &j,
            range: 0..len,
        })
    }
}

/// Consumer that expands each outer item `a` across a shared inner buffer,
/// yielding `(a, b)` for every `b`. A split at outer index `k` becomes an output
/// split at `k * inner.len()`, so exact/preallocating consumers stay exact while
/// the outer input is never buffered.
struct ExpandConsumer<C, B> {
    base: C,
    inner: Arc<[B]>,
}

impl<A, B, C> Consumer<A> for ExpandConsumer<C, B>
where
    A: Clone + Send,
    B: Clone + Send + Sync,
    C: Consumer<(A, B)>,
{
    type Folder = ExpandFolder<C::Folder, B>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        // `index` counts outer items; each expands to `inner.len()` outputs.
        let output_index = index
            .checked_mul(self.inner.len())
            .expect("cartesian_product length overflows usize");
        let (left, right, reducer) = self.base.split_at(output_index);
        (
            ExpandConsumer {
                base: left,
                inner: Arc::clone(&self.inner),
            },
            ExpandConsumer {
                base: right,
                inner: self.inner,
            },
            reducer,
        )
    }

    fn into_folder(self) -> Self::Folder {
        ExpandFolder {
            base: self.base.into_folder(),
            inner: self.inner,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<A, B, C> UnindexedConsumer<A> for ExpandConsumer<C, B>
where
    A: Clone + Send,
    B: Clone + Send + Sync,
    C: UnindexedConsumer<(A, B)>,
{
    fn split_off_left(&self) -> Self {
        ExpandConsumer {
            base: self.base.split_off_left(),
            inner: Arc::clone(&self.inner),
        }
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct ExpandFolder<F, B> {
    base: F,
    inner: Arc<[B]>,
}

impl<A, B, F> Folder<A> for ExpandFolder<F, B>
where
    A: Clone,
    B: Clone,
    F: Folder<(A, B)>,
{
    type Result = F::Result;

    fn consume(self, item: A) -> Self {
        let ExpandFolder { mut base, inner } = self;
        for b in inner.iter() {
            if base.full() {
                break;
            }
            base = base.consume((item.clone(), b.clone()));
        }
        ExpandFolder { base, inner }
    }

    fn complete(self) -> Self::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

/// Producer for the indexed path, indexing flat positions `k` in `range` as
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
