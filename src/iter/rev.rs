use super::internal::*;
use super::*;
use std::iter;

pub struct Rev<I: IndexedParallelIterator> {
    base: I,
}

/// Create a new `Rev` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I>(base: I) -> Rev<I>
    where I: IndexedParallelIterator
{
    Rev { base: base }
}

impl<I> ParallelIterator for Rev<I>
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

impl<I> IndexedParallelIterator for Rev<I>
    where I: IndexedParallelIterator
{
    fn drive<C, S>(self, consumer: C, scheduler: S) -> C::Result
        where C: Consumer<Self::Item>, S: Scheduler
    {
        scheduler.execute(self, consumer)
    }

    fn len(&mut self) -> usize {
        self.base.len()
    }

    fn with_producer<CB, S>(mut self, callback: CB, scheduler: S) -> CB::Output
        where CB: ProducerCallback<Self::Item>, S: Scheduler,
    {
        let len = self.base.len();
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           len: len,
                                       }, scheduler);

        struct Callback<CB> {
            callback: CB,
            len: usize,
        }

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<T>
        {
            type Output = CB::Output;
            fn callback<P, S>(self, base: P, scheduler: S) -> CB::Output
                where P: Producer<Item = T>, S: Scheduler,
            {
                let producer = RevProducer {
                    base: base,
                    len: self.len,
                };
                self.callback.callback(producer, scheduler)
            }
        }
    }
}

struct RevProducer<P> {
    base: P,
    len: usize,
}

impl<P> Producer for RevProducer<P>
    where P: Producer
{
    type Item = P::Item;
    type IntoIter = iter::Rev<P::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter().rev()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(self.len - index);
        (RevProducer {
             base: right,
             len: index,
         },
         RevProducer {
             base: left,
             len: self.len - index,
         })
    }
}
