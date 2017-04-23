use super::internal::*;
use super::*;

use std::iter;


/// `Map` is an iterator that transforms the elements of an underlying iterator.
///
/// This struct is created by the [`map()`] method on [`ParallelIterator`]
///
/// [`map()`]: trait.ParallelIterator.html#method.map
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct Map<I: ParallelIterator, F> {
    base: I,
    map_op: F,
}

/// Create a new `Map` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I, F>(base: I, map_op: F) -> Map<I, F>
where
    I: ParallelIterator,
{
    Map {
        base: base,
        map_op: map_op,
    }
}

impl<I, F, R> ParallelIterator for Map<I, F>
where
    I: ParallelIterator,
    F: Fn(I::Item) -> R + Sync,
    R: Send,
{
    type Item = F::Output;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let consumer1 = MapConsumer::new(consumer, &self.map_op);
        self.base.drive_unindexed(consumer1)
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}

impl<I, F, R> IndexedParallelIterator for Map<I, F>
where
    I: IndexedParallelIterator,
    F: Fn(I::Item) -> R + Sync,
    R: Send,
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        let consumer1 = MapConsumer::new(consumer, &self.map_op);
        self.base.drive(consumer1)
    }

    fn len(&mut self) -> usize {
        self.base.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        return self.base
                   .with_producer(
            Callback {
                callback: callback,
                map_op: self.map_op,
            },
        );

        struct Callback<CB, F> {
            callback: CB,
            map_op: F,
        }

        impl<T, F, R, CB> ProducerCallback<T> for Callback<CB, F>
        where
            CB: ProducerCallback<R>,
            F: Fn(T) -> R + Sync,
            R: Send,
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
            where
                P: Producer<Item = T>,
            {
                let producer = MapProducer {
                    base: base,
                    map_op: &self.map_op,
                };
                self.callback.callback(producer)
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct MapProducer<'f, P, F: 'f> {
    base: P,
    map_op: &'f F,
}

impl<'f, P, F, R> Producer for MapProducer<'f, P, F>
where
    P: Producer,
    F: Fn(P::Item) -> R + Sync,
    R: Send,
{
    type Item = F::Output;
    type IntoIter = iter::Map<P::IntoIter, &'f F>;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter().map(self.map_op)
    }

    fn min_len(&self) -> usize {
        self.base.min_len()
    }
    fn max_len(&self) -> usize {
        self.base.max_len()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapProducer {
             base: left,
             map_op: self.map_op,
         },
         MapProducer {
             base: right,
             map_op: self.map_op,
         })
    }
}


/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct MapConsumer<'f, C, F: 'f> {
    base: C,
    map_op: &'f F,
}

impl<'f, C, F> MapConsumer<'f, C, F> {
    fn new(base: C, map_op: &'f F) -> Self {
        MapConsumer {
            base: base,
            map_op: map_op,
        }
    }
}

impl<'f, T, R, C, F> Consumer<T> for MapConsumer<'f, C, F>
where
    C: Consumer<F::Output>,
    F: Fn(T) -> R + Sync,
    R: Send,
{
    type Folder = MapFolder<'f, C::Folder, F>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (MapConsumer::new(left, self.map_op), MapConsumer::new(right, self.map_op), reducer)
    }

    fn into_folder(self) -> Self::Folder {
        MapFolder {
            base: self.base.into_folder(),
            map_op: self.map_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'f, T, R, C, F> UnindexedConsumer<T> for MapConsumer<'f, C, F>
where
    C: UnindexedConsumer<F::Output>,
    F: Fn(T) -> R + Sync,
    R: Send,
{
    fn split_off_left(&self) -> Self {
        MapConsumer::new(self.base.split_off_left(), &self.map_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct MapFolder<'f, C, F: 'f> {
    base: C,
    map_op: &'f F,
}

impl<'f, T, R, C, F> Folder<T> for MapFolder<'f, C, F>
where
    C: Folder<F::Output>,
    F: Fn(T) -> R,
{
    type Result = C::Result;

    fn consume(self, item: T) -> Self {
        let mapped_item = (self.map_op)(item);
        MapFolder {
            base: self.base.consume(mapped_item),
            map_op: self.map_op,
        }
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
