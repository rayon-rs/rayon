use super::internal::*;
use super::*;
use std::cmp;

/// `MinLen` is an iterator that imposes a minimum length on iterator splits.
/// This struct is created by the [`min_len()`] method on [`IndexedParallelIterator`]
///
/// [`min_len()`]: trait.IndexedParallelIterator.html#method.min_len
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
pub struct MinLen<I: IndexedParallelIterator> {
    base: I,
    min: usize,
}

/// Create a new `MinLen` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new_min_len<I>(base: I, min: usize) -> MinLen<I>
    where I: IndexedParallelIterator
{
    MinLen {
        base: base,
        min: min,
    }
}

impl<I> ParallelIterator for MinLen<I>
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

impl<I> IndexedParallelIterator for MinLen<I>
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

    fn with_producer<CB, S>(self, callback: CB, scheduler: S) -> CB::Output
        where CB: ProducerCallback<Self::Item>, S: Scheduler,
    {
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           min: self.min,
                                       }, scheduler);

        struct Callback<CB> {
            callback: CB,
            min: usize,
        }

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<T>
        {
            type Output = CB::Output;
            fn callback<P, S>(self, base: P, scheduler: S) -> CB::Output
                where P: Producer<Item = T>, S: Scheduler
            {
                let producer = MinLenProducer {
                    base: base,
                    min: self.min,
                };
                self.callback.callback(producer, scheduler)
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// `MinLenProducer` implementation

struct MinLenProducer<P> {
    base: P,
    min: usize,
}

impl<P> Producer for MinLenProducer<P>
    where P: Producer
{
    type Item = P::Item;
    type IntoIter = P::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MinLenProducer {
             base: left,
             min: self.min,
         },
         MinLenProducer {
             base: right,
             min: self.min,
         })
    }
}


/// `MaxLen` is an iterator that imposes a maximum length on iterator splits.
/// This struct is created by the [`max_len()`] method on [`IndexedParallelIterator`]
///
/// [`max_len()`]: trait.IndexedParallelIterator.html#method.max_len
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
pub struct MaxLen<I: IndexedParallelIterator> {
    base: I,
    max: usize,
}

/// Create a new `MaxLen` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new_max_len<I>(base: I, max: usize) -> MaxLen<I>
    where I: IndexedParallelIterator
{
    MaxLen {
        base: base,
        max: max,
    }
}

impl<I> ParallelIterator for MaxLen<I>
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

impl<I> IndexedParallelIterator for MaxLen<I>
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

    fn with_producer<CB, S>(self, callback: CB, scheduler: S) -> CB::Output
        where CB: ProducerCallback<Self::Item>, S: Scheduler,
    {
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           max: self.max,
                                       }, scheduler);

        struct Callback<CB> {
            callback: CB,
            max: usize,
        }

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<T>
        {
            type Output = CB::Output;
            fn callback<P, S>(self, base: P, scheduler: S) -> CB::Output
                where P: Producer<Item = T>, S: Scheduler
            {
                let producer = MaxLenProducer {
                    base: base,
                    max: self.max,
                };
                self.callback.callback(producer, scheduler)
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// `MaxLenProducer` implementation

struct MaxLenProducer<P> {
    base: P,
    max: usize,
}

impl<P> Producer for MaxLenProducer<P>
    where P: Producer
{
    type Item = P::Item;
    type IntoIter = P::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MaxLenProducer {
             base: left,
             max: self.max,
         },
         MaxLenProducer {
             base: right,
             max: self.max,
         })
    }
}
