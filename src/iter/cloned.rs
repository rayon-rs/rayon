use super::internal::*;
use super::*;

use std::iter;

/// `Cloned` is an iterator that clones the elements of an underlying iterator.
///
/// This struct is created by the [`cloned()`] method on [`ParallelIterator`]
///
/// [`cloned()`]: trait.ParallelIterator.html#method.cloned
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct Cloned<I: ParallelIterator> {
    base: I,
}

/// Create a new `Cloned` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I>(base: I) -> Cloned<I>
    where I: ParallelIterator
{
    Cloned { base: base }
}

impl<'a, T, I> ParallelIterator for Cloned<I>
    where I: ParallelIterator<Item = &'a T>,
          T: 'a + Clone + Send + Sync
{
    type Item = T;
    type Scheduler = I::Scheduler;

    fn drive_unindexed<C, S>(self, consumer: C, scheduler: S) -> C::Result
        where C: UnindexedConsumer<Self::Item>, S: Scheduler,
    {
        let consumer1 = ClonedConsumer::new(consumer);
        self.base.drive_unindexed(consumer1, scheduler)
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}

impl<'a, T, I> IndexedParallelIterator for Cloned<I>
    where I: IndexedParallelIterator<Item = &'a T>,
          T: 'a + Clone + Send + Sync
{
    fn drive<C, S>(self, consumer: C, scheduler: S) -> C::Result
        where C: Consumer<Self::Item>, S: Scheduler,
    {
        let consumer1 = ClonedConsumer::new(consumer);
        self.base.drive(consumer1, scheduler)
    }

    fn len(&mut self) -> usize {
        self.base.len()
    }

    fn with_producer<CB, S>(self, callback: CB, scheduler: S) -> CB::Output
        where CB: ProducerCallback<Self::Item>, S: Scheduler,
    {
        return self.base.with_producer(Callback { callback: callback }, scheduler);

        struct Callback<CB> {
            callback: CB,
        }

        impl<'a, T, CB> ProducerCallback<&'a T> for Callback<CB>
            where CB: ProducerCallback<T>,
                  T: 'a + Clone + Send
        {
            type Output = CB::Output;

            fn callback<P, S>(self, base: P, scheduler: S) -> CB::Output
                where P: Producer<Item = &'a T>, S: Scheduler
            {
                let producer = ClonedProducer { base: base };
                self.callback.callback(producer, scheduler)
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct ClonedProducer<P> {
    base: P,
}

impl<'a, T, P> Producer for ClonedProducer<P>
    where P: Producer<Item = &'a T>,
          T: 'a + Clone
{
    type Item = T;
    type IntoIter = iter::Cloned<P::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter().cloned()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (ClonedProducer { base: left }, ClonedProducer { base: right })
    }
}


/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct ClonedConsumer<C> {
    base: C,
}

impl<C> ClonedConsumer<C> {
    fn new(base: C) -> Self {
        ClonedConsumer { base: base }
    }
}

impl<'a, T, C> Consumer<&'a T> for ClonedConsumer<C>
    where C: Consumer<T>,
          T: 'a + Clone
{
    type Folder = ClonedFolder<C::Folder>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (ClonedConsumer::new(left), ClonedConsumer::new(right), reducer)
    }

    fn into_folder(self) -> Self::Folder {
        ClonedFolder { base: self.base.into_folder() }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'a, T, C> UnindexedConsumer<&'a T> for ClonedConsumer<C>
    where C: UnindexedConsumer<T>,
          T: 'a + Clone
{
    fn split_off_left(&self) -> Self {
        ClonedConsumer::new(self.base.split_off_left())
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}


struct ClonedFolder<F> {
    base: F,
}

impl<'a, T, F> Folder<&'a T> for ClonedFolder<F>
    where F: Folder<T>,
          T: 'a + Clone
{
    type Result = F::Result;

    fn consume(self, item: &'a T) -> Self {
        ClonedFolder { base: self.base.consume(item.clone()) }
    }

    fn complete(self) -> F::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
