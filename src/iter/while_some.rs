use std::sync::atomic::{AtomicBool, Ordering};
use super::internal::*;
use super::*;

/// `WhileSome` is an iterator that yields the `Some` elements of an iterator,
/// halting as soon as any `None` is produced.
///
/// This struct is created by the [`while_some()`] method on [`ParallelIterator`]
///
/// [`while_some()`]: trait.ParallelIterator.html#method.while_some
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct WhileSome<I: ParallelIterator> {
    base: I,
}

/// Create a new `WhileSome` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I>(base: I) -> WhileSome<I>
    where I: ParallelIterator
{
    WhileSome { base: base }
}

impl<I, T> ParallelIterator for WhileSome<I>
    where I: ParallelIterator<Item = Option<T>>,
          T: Send
{
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let full = AtomicBool::new(false);
        let consumer1 = WhileSomeConsumer {
            base: consumer,
            full: &full,
        };
        self.base.drive_unindexed(consumer1)
    }
}


/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct WhileSomeConsumer<'f, C> {
    base: C,
    full: &'f AtomicBool,
}

impl<'f, T, C> Consumer<Option<T>> for WhileSomeConsumer<'f, C>
    where C: Consumer<T>,
          T: Send
{
    type Folder = WhileSomeFolder<'f, C::Folder>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (WhileSomeConsumer { base: left, ..self },
         WhileSomeConsumer { base: right, ..self },
         reducer)
    }

    fn into_folder(self) -> Self::Folder {
        WhileSomeFolder {
            base: self.base.into_folder(),
            full: self.full,
        }
    }

    fn full(&self) -> bool {
        self.full.load(Ordering::Relaxed) || self.base.full()
    }
}

impl<'f, T, C> UnindexedConsumer<Option<T>> for WhileSomeConsumer<'f, C>
    where C: UnindexedConsumer<T>,
          T: Send
{
    fn split_off_left(&self) -> Self {
        WhileSomeConsumer { base: self.base.split_off_left(), ..*self }
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct WhileSomeFolder<'f, C> {
    base: C,
    full: &'f AtomicBool,
}

impl<'f, T, C> Folder<Option<T>> for WhileSomeFolder<'f, C>
    where C: Folder<T>
{
    type Result = C::Result;

    fn consume(mut self, item: Option<T>) -> Self {
        match item {
            Some(item) => self.base = self.base.consume(item),
            None => self.full.store(true, Ordering::Relaxed),
        }
        self
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.full.load(Ordering::Relaxed) || self.base.full()
    }
}
