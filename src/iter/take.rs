use super::internal::*;
use super::*;
use std::cmp::min;

/// `Take` is an iterator that iterates over the first `n` elements.
/// This struct is created by the [`take()`] method on [`ParallelIterator`]
///
/// [`take()`]: trait.ParallelIterator.html#method.take
/// [`ParallelIterator`]: trait.ParallelIterator.html
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
    type Scheduler = I::Scheduler;

    fn drive_unindexed<C, S>(self, consumer: C, scheduler: S) -> C::Result
        where C: UnindexedConsumer<Self::Item>, S: Scheduler,
    {
        scheduler.execute(self, consumer)
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

    fn drive<C, S>(self, consumer: C, scheduler: S) -> C::Result
        where C: Consumer<Self::Item>, S: Scheduler
    {
        scheduler.execute(self, consumer)
    }

    fn with_producer<CB, S>(self, callback: CB, scheduler: S) -> CB::Output
        where CB: ProducerCallback<Self::Item>, S: Scheduler,
    {
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           n: self.n,
                                       }, scheduler);

        struct Callback<CB> {
            callback: CB,
            n: usize,
        }

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<T>
        {
            type Output = CB::Output;
            fn callback<P, S>(self, base: P, scheduler: S) -> CB::Output
                where P: Producer<Item = T>, S: Scheduler,
            {
                let (producer, _) = base.split_at(self.n);
                self.callback.callback(producer, scheduler)
            }
        }
    }
}
