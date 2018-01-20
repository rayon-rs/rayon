use ::math::div_round_up;
use super::plumbing::*;
use super::*;

#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct Chunks<I>
    where I: IndexedParallelIterator
{
    size: usize,
    i: I,
}

/// Create a new `Chunks` iterator
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I>(i: I, size: usize) -> Chunks<I>
    where I: IndexedParallelIterator
{
    Chunks { i: i, size: size }
}

impl<I> ParallelIterator for Chunks<I>
    where I: IndexedParallelIterator
{
    type Item = Vec<I::Item>;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: Consumer<Vec<I::Item>>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I> IndexedParallelIterator for Chunks<I>
    where I: IndexedParallelIterator
{
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        div_round_up(self.i.len(), self.size)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.i.with_producer(Callback {
            size: self.size,
            callback: callback,
        });

        struct Callback<CB> {
            size: usize,
            callback: CB,
        }

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<Vec<T>>
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = T>
            {
                self.callback.callback(ChunkProducer {
                    chunk_size: self.size,
                    base: base,
                })
            }
        }
    }
}

struct ChunkProducer<P>
    where P: Producer
{
    chunk_size: usize,
    base: P,
}

impl<P> Producer for ChunkProducer<P>
    where P: Producer
{
    type Item = Vec<P::Item>;
    type IntoIter = ChunkSeq<P::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        ChunkSeq {
            chunk_size: self.chunk_size,
            inner: self.base.into_iter()
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.base.split_at(elem_index);
        (ChunkProducer {
            chunk_size: self.chunk_size,
            base: left,
        },
        ChunkProducer {
            chunk_size: self.chunk_size,
            base: right,
        })
    }

    fn min_len(&self) -> usize {
        div_round_up(self.base.min_len(), self.chunk_size)
    }

    fn max_len(&self) -> usize {
        self.base.max_len() / self.chunk_size
    }
}

struct ChunkSeq<I> {
    chunk_size: usize,
    inner: I,
}

impl<I> Iterator for ChunkSeq<I>
    where I: Iterator + ExactSizeIterator
{
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.len() > 0 {
            Some(self.inner.by_ref().take(self.chunk_size).collect())
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<I> ExactSizeIterator for ChunkSeq<I>
    where I: Iterator + ExactSizeIterator
{
    #[inline]
    fn len(&self) -> usize {
        div_round_up(self.inner.len(), self.chunk_size)
    }
}

impl<I> DoubleEndedIterator for ChunkSeq<I>
    where I: Iterator + ExactSizeIterator + DoubleEndedIterator
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.inner.len() == 0 {
            return None
        }
        let mut last_size = self.inner.len() % self.chunk_size;
        if last_size == 0 {
            last_size = self.chunk_size;
        }
        let mut chunk: Vec<_> = self.inner.by_ref().rev().take(last_size).collect();
        // the elements themselves shouldn't be reversed.
        chunk.reverse();
        Some(chunk)
    }
}
