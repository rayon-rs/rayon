use std::cmp::min;
use std::fmt::{self, Debug};

use super::plumbing::*;
use super::*;
use crate::math::div_round_up;

/// `FoldChunksWith` is an iterator that groups elements of an underlying iterator and applies a
/// function over them, producing a single value for each group.
///
/// This struct is created by the [`fold_chunks_with()`] method on [`IndexedParallelIterator`]
///
/// [`fold_chunks_with()`]: trait.IndexedParallelIterator.html#method.fold_chunks
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct FoldChunksWith<I, U, F>
where
    I: IndexedParallelIterator,
{
    base: I,
    chunk_size: usize,
    item: U,
    fold_op: F,
}

impl<I: IndexedParallelIterator + Debug, U: Debug, F> Debug for FoldChunksWith<I, U, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fold")
            .field("base", &self.base)
            .field("chunk_size", &self.chunk_size)
            .field("item", &self.item)
            .finish()
    }
}

impl<I, U, F> FoldChunksWith<I, U, F>
where
    I: IndexedParallelIterator,
    U: Send + Clone,
    F: Fn(U, I::Item) -> U + Send + Sync,
{
    /// Creates a new `FoldChunksWith` iterator
    pub(super) fn new(base: I, chunk_size: usize, item: U, fold_op: F) -> Self {
        FoldChunksWith {
            base,
            chunk_size,
            item,
            fold_op,
        }
    }
}

impl<I, U, F> ParallelIterator for FoldChunksWith<I, U, F>
where
    I: IndexedParallelIterator,
    U: Send + Clone,
    F: Fn(U, I::Item) -> U + Send + Sync,
{
    type Item = U;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<U>,
    {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I, U, F> IndexedParallelIterator for FoldChunksWith<I, U, F>
where
    I: IndexedParallelIterator,
    U: Send + Clone,
    F: Fn(U, I::Item) -> U + Send + Sync,
{
    fn len(&self) -> usize {
        div_round_up(self.base.len(), self.chunk_size)
    }

    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        let len = self.base.len();
        return self.base.with_producer(Callback {
            chunk_size: self.chunk_size,
            len,
            item: self.item,
            fold_op: self.fold_op,
            callback,
        });

        struct Callback<CB, T, F> {
            chunk_size: usize,
            len: usize,
            item: T,
            fold_op: F,
            callback: CB,
        }

        impl<T, U, F, CB> ProducerCallback<T> for Callback<CB, U, F>
        where
            CB: ProducerCallback<U>,
            U: Send + Clone,
            F: Fn(U, T) -> U + Send + Sync,
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
            where
                P: Producer<Item = T>,
            {
                self.callback.callback(FoldChunksWithProducer {
                    chunk_size: self.chunk_size,
                    len: self.len,
                    item: self.item,
                    fold_op: &self.fold_op,
                    base,
                })
            }
        }
    }
}

struct FoldChunksWithProducer<'f, P, U, F>
where
    P: Producer,
{
    chunk_size: usize,
    len: usize,
    item: U,
    fold_op: &'f F,
    base: P,
}

impl<'f, P, U, F> Producer for FoldChunksWithProducer<'f, P, U, F>
where
    P: Producer,
    U: Send + Clone,
    F: Fn(U, P::Item) -> U + Send + Sync,
{
    type Item = F::Output;
    type IntoIter = FoldChunksWithSeq<'f, P, U, F>;

    fn into_iter(self) -> Self::IntoIter {
        FoldChunksWithSeq {
            chunk_size: self.chunk_size,
            len: self.len,
            item: self.item,
            fold_op: self.fold_op,
            inner: if self.len > 0 { Some(self.base) } else { None },
        }
    }

    fn min_len(&self) -> usize {
        div_round_up(self.base.min_len(), self.chunk_size)
    }

    fn max_len(&self) -> usize {
        self.base.max_len() / self.chunk_size
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = min(index * self.chunk_size, self.len);
        let (left, right) = self.base.split_at(elem_index);
        (
            FoldChunksWithProducer {
                chunk_size: self.chunk_size,
                len: elem_index,
                item: self.item.clone(),
                fold_op: self.fold_op,
                base: left,
            },
            FoldChunksWithProducer {
                chunk_size: self.chunk_size,
                len: self.len - elem_index,
                item: self.item,
                fold_op: self.fold_op,
                base: right,
            },
        )
    }
}

struct FoldChunksWithSeq<'f, P, U, F> {
    chunk_size: usize,
    len: usize,
    item: U,
    fold_op: &'f F,
    inner: Option<P>,
}

impl<'f, P, U, F> Iterator for FoldChunksWithSeq<'f, P, U, F>
where
    P: Producer,
    U: Send + Clone,
    F: Fn(U, P::Item) -> U + Send + Sync,
{
    type Item = U;

    fn next(&mut self) -> Option<Self::Item> {
        let producer = self.inner.take()?;
        if self.len > self.chunk_size {
            let (left, right) = producer.split_at(self.chunk_size);
            self.inner = Some(right);
            self.len -= self.chunk_size;
            let chunk = left.into_iter();
            Some(chunk.fold(self.item.clone(), self.fold_op))
        } else {
            debug_assert!(self.len > 0);
            self.len = 0;
            let chunk = producer.into_iter();
            Some(chunk.fold(self.item.clone(), self.fold_op))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'f, P, U, F> ExactSizeIterator for FoldChunksWithSeq<'f, P, U, F>
where
    P: Producer,
    U: Send + Clone,
    F: Fn(U, P::Item) -> U + Send + Sync,
{
    #[inline]
    fn len(&self) -> usize {
        div_round_up(self.len, self.chunk_size)
    }
}

impl<'f, P, U, F> DoubleEndedIterator for FoldChunksWithSeq<'f, P, U, F>
where
    P: Producer,
    U: Send + Clone,
    F: Fn(U, P::Item) -> U + Send + Sync,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let producer = self.inner.take()?;
        if self.len > self.chunk_size {
            let mut size = self.len % self.chunk_size;
            if size == 0 {
                size = self.chunk_size;
            }
            let (left, right) = producer.split_at(self.len - size);
            self.inner = Some(left);
            self.len -= size;
            let chunk = right.into_iter();
            Some(chunk.fold(self.item.clone(), self.fold_op))
        } else {
            debug_assert!(self.len > 0);
            self.len = 0;
            let chunk = producer.into_iter();
            Some(chunk.fold(self.item.clone(), self.fold_op))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn check_fold_chunks_with() {
        let words = "bishbashbosh!"
            .chars()
            .collect::<Vec<_>>()
            .into_par_iter()
            .fold_chunks_with(4, String::new(), |mut s, c| {
                s.push(c);
                s
            })
            .collect::<Vec<_>>();

        assert_eq!(words, vec!["bish", "bash", "bosh", "!"]);
    }
}
