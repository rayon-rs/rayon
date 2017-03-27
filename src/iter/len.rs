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

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I> BoundedParallelIterator for MinLen<I>
    where I: IndexedParallelIterator
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
}

impl<I> ExactParallelIterator for MinLen<I>
    where I: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<I> IndexedParallelIterator for MinLen<I>
    where I: IndexedParallelIterator
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           min: self.min,
                                       });

        struct Callback<CB> {
            callback: CB,
            min: usize,
        }

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<T>
        {
            type Output = CB::Output;
            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = T>
            {
                let producer = MinLenProducer {
                    base: base,
                    min: self.min,
                };
                self.callback.callback(producer)
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

    fn min_len(&self) -> usize {
        cmp::max(self.min, self.base.min_len())
    }

    fn max_len(&self) -> usize {
        self.base.max_len()
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

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I> BoundedParallelIterator for MaxLen<I>
    where I: IndexedParallelIterator
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
}

impl<I> ExactParallelIterator for MaxLen<I>
    where I: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<I> IndexedParallelIterator for MaxLen<I>
    where I: IndexedParallelIterator
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           max: self.max,
                                       });

        struct Callback<CB> {
            callback: CB,
            max: usize,
        }

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<T>
        {
            type Output = CB::Output;
            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = T>
            {
                let producer = MaxLenProducer {
                    base: base,
                    max: self.max,
                };
                self.callback.callback(producer)
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

    fn min_len(&self) -> usize {
        self.base.min_len()
    }

    fn max_len(&self) -> usize {
        cmp::min(self.max, self.base.max_len())
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
