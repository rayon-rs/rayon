use super::plumbing::*;
use super::*;

/// `InterleaveShortest` is an iterator that works similarly to
/// `Interleave`, but this version stops returning elements once one
/// of the iterators run out.
///
/// This struct is created by the [`interleave_shortest()`] method on
/// [`IndexedParallelIterator`].
///
/// [`interleave_shortest()`]: trait.IndexedParallelIterator.html#method.interleave_shortest
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct InterleaveShortest<I, J>
    where I: IndexedParallelIterator,
          J: IndexedParallelIterator<Item = I::Item>
{
    interleave: Interleave<Take<I>, Take<J>>
}

/// Create a new `InterleaveShortest` iterator
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I, J>(i: I, j: J) -> InterleaveShortest<I, J>
    where I: IndexedParallelIterator,
          J: IndexedParallelIterator<Item = I::Item>
{
    InterleaveShortest {
        interleave: if i.len() <= j.len() {
            // take equal lengths from both iterators
            let n = i.len();
            i.take(n).interleave(j.take(n))
        } else {
            // take one extra item from the first iterator
            let n = j.len();
            i.take(n + 1).interleave(j.take(n))
        }
    }
}


impl<I, J> ParallelIterator for InterleaveShortest<I, J>
    where I: IndexedParallelIterator,
          J: IndexedParallelIterator<Item = I::Item>
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: Consumer<I::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I, J> IndexedParallelIterator for InterleaveShortest<I, J>
    where I: IndexedParallelIterator,
          J: IndexedParallelIterator<Item = I::Item>,
{
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.interleave.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        self.interleave.with_producer(callback)
    }
}
