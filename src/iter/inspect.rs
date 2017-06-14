use super::internal::*;
use super::*;

use std::iter;


/// `Inspect` is an iterator that calls a function with a reference to each
/// element before yielding it.
///
/// This struct is created by the [`inspect()`] method on [`ParallelIterator`]
///
/// [`inspect()`]: trait.ParallelIterator.html#method.inspect
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Inspect<I: ParallelIterator, F> {
    base: I,
    inspect_op: F,
}

/// Create a new `Inspect` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I, F>(base: I, inspect_op: F) -> Inspect<I, F>
    where I: ParallelIterator
{
    Inspect {
        base: base,
        inspect_op: inspect_op,
    }
}

impl<I, F> ParallelIterator for Inspect<I, F>
    where I: ParallelIterator,
          F: Fn(&I::Item) + Sync + Send
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = InspectConsumer::new(consumer, &self.inspect_op);
        self.base.drive_unindexed(consumer1)
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}

impl<I, F> IndexedParallelIterator for Inspect<I, F>
    where I: IndexedParallelIterator,
          F: Fn(&I::Item) + Sync + Send
{
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        let consumer1 = InspectConsumer::new(consumer, &self.inspect_op);
        self.base.drive(consumer1)
    }

    fn len(&mut self) -> usize {
        self.base.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base
                   .with_producer(Callback {
                                      callback: callback,
                                      inspect_op: self.inspect_op,
                                  });

        struct Callback<CB, F> {
            callback: CB,
            inspect_op: F,
        }

        impl<T, F, CB> ProducerCallback<T> for Callback<CB, F>
            where CB: ProducerCallback<T>,
                  F: Fn(&T) + Sync
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = T>
            {
                let producer = InspectProducer {
                    base: base,
                    inspect_op: &self.inspect_op,
                };
                self.callback.callback(producer)
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct InspectProducer<'f, P, F: 'f> {
    base: P,
    inspect_op: &'f F,
}

impl<'f, P, F> Producer for InspectProducer<'f, P, F>
    where P: Producer,
          F: Fn(&P::Item) + Sync
{
    type Item = P::Item;
    type IntoIter = iter::Inspect<P::IntoIter, &'f F>;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter().inspect(self.inspect_op)
    }

    fn min_len(&self) -> usize {
        self.base.min_len()
    }

    fn max_len(&self) -> usize {
        self.base.max_len()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (InspectProducer {
             base: left,
             inspect_op: self.inspect_op,
         },
         InspectProducer {
             base: right,
             inspect_op: self.inspect_op,
         })
    }
}


/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct InspectConsumer<'f, C, F: 'f> {
    base: C,
    inspect_op: &'f F,
}

impl<'f, C, F> InspectConsumer<'f, C, F> {
    fn new(base: C, inspect_op: &'f F) -> Self {
        InspectConsumer {
            base: base,
            inspect_op: inspect_op,
        }
    }
}

impl<'f, T, C, F> Consumer<T> for InspectConsumer<'f, C, F>
    where C: Consumer<T>,
          F: Fn(&T) + Sync
{
    type Folder = InspectFolder<'f, C::Folder, F>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (InspectConsumer::new(left, self.inspect_op),
         InspectConsumer::new(right, self.inspect_op),
         reducer)
    }

    fn into_folder(self) -> Self::Folder {
        InspectFolder {
            base: self.base.into_folder(),
            inspect_op: self.inspect_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'f, T, C, F> UnindexedConsumer<T> for InspectConsumer<'f, C, F>
    where C: UnindexedConsumer<T>,
          F: Fn(&T) + Sync
{
    fn split_off_left(&self) -> Self {
        InspectConsumer::new(self.base.split_off_left(), &self.inspect_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct InspectFolder<'f, C, F: 'f> {
    base: C,
    inspect_op: &'f F,
}

impl<'f, T, C, F> Folder<T> for InspectFolder<'f, C, F>
    where C: Folder<T>,
          F: Fn(&T)
{
    type Result = C::Result;

    fn consume(self, item: T) -> Self {
        (self.inspect_op)(&item);
        InspectFolder {
            base: self.base.consume(item),
            inspect_op: self.inspect_op,
        }
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
