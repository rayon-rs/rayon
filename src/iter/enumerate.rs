use super::internal::*;
use super::*;
use std::iter;
use std::ops::Range;
use std::usize;

/// `Enumerate` is an iterator that returns the current count along with the element.
/// This struct is created by the [`enumerate()`] method on [`ParallelIterator`]
///
/// [`enumerate()`]: trait.ParallelIterator.html#method.enumerate
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct Enumerate<I: IndexedParallelIterator> {
    base: I,
}

/// Create a new `Enumerate` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I>(base: I) -> Enumerate<I>
    where I: IndexedParallelIterator
{
    Enumerate { base: base }
}

impl<I> ParallelIterator for Enumerate<I>
    where I: IndexedParallelIterator
{
    type Item = (usize, I::Item);
    type Scheduler = I::Scheduler;

    fn drive_unindexed<C, S>(self,
                             consumer: C,
                             scheduler: S)
                             -> C::Result
        where C: UnindexedConsumer<Self::Item>, S: Scheduler,
    {
        scheduler.execute(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I> IndexedParallelIterator for Enumerate<I>
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
        return self.base.with_producer(Callback { callback: callback }, scheduler);

        struct Callback<CB> {
            callback: CB,
        }

        impl<I, CB> ProducerCallback<I> for Callback<CB>
            where CB: ProducerCallback<(usize, I)>
        {
            type Output = CB::Output;
            fn callback<P, S>(self, base: P, scheduler: S) -> CB::Output
                where P: Producer<Item = I>, S: Scheduler,
            {
                let producer = EnumerateProducer {
                    base: base,
                    offset: 0,
                };
                self.callback.callback(producer, scheduler)
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Producer implementation

struct EnumerateProducer<P> {
    base: P,
    offset: usize,
}

impl<P> Producer for EnumerateProducer<P>
    where P: Producer
{
    type Item = (usize, P::Item);
    type IntoIter = iter::Zip<Range<usize>, P::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        // Enumerate only works for IndexedParallelIterators. Since those
        // have a max length of usize::MAX, their max index is
        // usize::MAX - 1, so the range 0..usize::MAX includes all
        // possible indices
        (self.offset..usize::MAX).zip(self.base.into_iter())
    }

    fn min_len(&self) -> usize {
        self.base.min_len()
    }
    fn max_len(&self) -> usize {
        self.base.max_len()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (EnumerateProducer {
             base: left,
             offset: self.offset,
         },
         EnumerateProducer {
             base: right,
             offset: self.offset + index,
         })
    }
}
