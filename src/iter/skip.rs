use super::internal::*;
use super::*;
use super::noop::NoopConsumer;
use std::cmp::min;

/// `Skip` is an iterator that skips over the first `n` elements.
/// This struct is created by the [`skip()`] method on [`ParallelIterator`]
///
/// [`skip()`]: trait.ParallelIterator.html#method.skip
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct Skip<I> {
    base: I,
    n: usize,
}

/// Create a new `Skip` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I>(mut base: I, n: usize) -> Skip<I>
    where I: IndexedParallelIterator
{
    let n = min(base.len(), n);
    Skip { base: base, n: n }
}

impl<I> ParallelIterator for Skip<I>
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

impl<I> IndexedParallelIterator for Skip<I>
    where I: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        self.base.len() - self.n
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
                let (before_skip, after_skip) = base.split_at(self.n);
                scheduler.execute_indexed(self.n, before_skip, NoopConsumer::new());
                self.callback.callback(after_skip, scheduler)
            }
        }
    }
}
