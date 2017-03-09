use super::internal::*;
use super::*;
use super::noop::NoopConsumer;
use std::cmp::min;

/// `Skip` is an iterator that skips over the first `n` elements.
/// This struct is created by the [`skip()`] method on [`ParallelIterator`]
///
/// [`skip()`]: trait.ParallelIterator.html#method.skip
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct Skip<M> {
    base: M,
    n: usize,
}

/// Create a new `Skip` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<M>(mut base: M, n: usize) -> Skip<M>
    where M: IndexedParallelIterator
{
    let n = min(base.len(), n);
    Skip { base: base, n: n }
}

impl<M> ParallelIterator for Skip<M>
    where M: IndexedParallelIterator
{
    type Item = M::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<M> ExactParallelIterator for Skip<M>
    where M: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        self.base.len() - self.n
    }
}

impl<M> BoundedParallelIterator for Skip<M>
    where M: IndexedParallelIterator
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
}

impl<M> IndexedParallelIterator for Skip<M>
    where M: IndexedParallelIterator
{
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

        impl<ITEM, CB> ProducerCallback<ITEM> for Callback<CB>
            where CB: ProducerCallback<ITEM>
        {
            type Output = CB::Output;
            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = ITEM>
            {
                let (before_skip, after_skip) = base.split_at(self.n);
                bridge_producer_consumer(self.n, before_skip, NoopConsumer::new());
                self.callback.callback(after_skip)
            }
        }
    }
}
