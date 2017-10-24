use iter::internal::*;
use iter::*;

use std;
use std::fmt;
use std::marker::PhantomData;

pub fn empty<T: Send>() -> Empty<T> {
    Empty { marker: PhantomData }
}

pub struct Empty<T: Send> {
    marker: PhantomData<T>,
}

impl<T: Send> Clone for Empty<T> {
    fn clone(&self) -> Self {
        empty()
    }
}

impl<T: Send> fmt::Debug for Empty<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Empty")
    }
}

impl<T: Send> ParallelIterator for Empty<T> {
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        self.drive(consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(0)
    }
}

impl<T: Send> IndexedParallelIterator for Empty<T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        consumer.into_folder().complete()
    }

    fn len(&mut self) -> usize {
        0
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(EmptyProducer(PhantomData))
    }
}

/// Private empty producer
struct EmptyProducer<T: Send>(PhantomData<T>);

impl<T: Send> Producer for EmptyProducer<T> {
    type Item = T;
    type IntoIter = std::iter::Empty<T>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::empty()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        debug_assert_eq!(index, 0);
        (self, EmptyProducer(PhantomData))
    }
}
