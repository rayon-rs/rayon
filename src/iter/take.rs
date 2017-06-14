use super::internal::*;
use super::*;
use std::cmp::min;

/// `Take` is an iterator that iterates over the first `n` elements.
/// This struct is created by the [`take()`] method on [`ParallelIterator`]
///
/// [`take()`]: trait.ParallelIterator.html#method.take
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Take<I> {
    base: I,
    n: usize,
}

/// Create a new `Take` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I>(mut base: I, n: usize) -> Take<I>
    where I: IndexedParallelIterator
{
    let n = min(base.len(), n);
    Take { base: base, n: n }
}

impl<I> ParallelIterator for Take<I>
    where I: IndexedParallelIterator
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I> IndexedParallelIterator for Take<I>
    where I: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        self.n
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           n: self.n,
                                       });

        struct Callback<CB> {
            callback: CB,
            n: usize,
        }

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<T>
        {
            type Output = CB::Output;
            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = T>
            {
                let (producer, _) = base.split_at(self.n);
                self.callback.callback(producer)
            }
        }
    }
}
