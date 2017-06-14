use super::internal::*;
use super::*;


/// `MapWith` is an iterator that transforms the elements of an underlying iterator.
///
/// This struct is created by the [`map_with()`] method on [`ParallelIterator`]
///
/// [`map_with()`]: trait.ParallelIterator.html#method.map_with
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct MapWith<I: ParallelIterator, T, F> {
    base: I,
    item: T,
    map_op: F,
}

/// Create a new `MapWith` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I, T, F>(base: I, item: T, map_op: F) -> MapWith<I, T, F>
    where I: ParallelIterator
{
    MapWith {
        base: base,
        item: item,
        map_op: map_op,
    }
}

impl<I, T, F, R> ParallelIterator for MapWith<I, T, F>
    where I: ParallelIterator,
          T: Send + Clone,
          F: Fn(&mut T, I::Item) -> R + Sync + Send,
          R: Send
{
    type Item = R;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = MapWithConsumer::new(consumer, self.item, &self.map_op);
        self.base.drive_unindexed(consumer1)
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}

impl<I, T, F, R> IndexedParallelIterator for MapWith<I, T, F>
    where I: IndexedParallelIterator,
          T: Send + Clone,
          F: Fn(&mut T, I::Item) -> R + Sync + Send,
          R: Send
{
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        let consumer1 = MapWithConsumer::new(consumer, self.item, &self.map_op);
        self.base.drive(consumer1)
    }

    fn len(&mut self) -> usize {
        self.base.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           item: self.item,
                                           map_op: self.map_op,
                                       });

        struct Callback<CB, U, F> {
            callback: CB,
            item: U,
            map_op: F,
        }

        impl<T, U, F, R, CB> ProducerCallback<T> for Callback<CB, U, F>
            where CB: ProducerCallback<R>,
                  U: Send + Clone,
                  F: Fn(&mut U, T) -> R + Sync,
                  R: Send
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = T>
            {
                let producer = MapWithProducer {
                    base: base,
                    item: self.item,
                    map_op: &self.map_op,
                };
                self.callback.callback(producer)
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct MapWithProducer<'f, P, U, F: 'f> {
    base: P,
    item: U,
    map_op: &'f F,
}

impl<'f, P, U, F, R> Producer for MapWithProducer<'f, P, U, F>
    where P: Producer,
          U: Send + Clone,
          F: Fn(&mut U, P::Item) -> R + Sync,
          R: Send
{
    type Item = R;
    type IntoIter = MapWithIter<'f, P::IntoIter, U, F>;

    fn into_iter(self) -> Self::IntoIter {
        MapWithIter {
            base: self.base.into_iter(),
            item: self.item,
            map_op: self.map_op,
        }
    }

    fn min_len(&self) -> usize {
        self.base.min_len()
    }
    fn max_len(&self) -> usize {
        self.base.max_len()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapWithProducer {
             base: left,
             item: self.item.clone(),
             map_op: self.map_op,
         },
         MapWithProducer {
             base: right,
             item: self.item,
             map_op: self.map_op,
         })
    }
}

struct MapWithIter<'f, I, U, F: 'f> {
    base: I,
    item: U,
    map_op: &'f F,
}

impl<'f, I, U, F, R> Iterator for MapWithIter<'f, I, U, F>
    where I: Iterator,
          U: Send + Clone,
          F: Fn(&mut U, I::Item) -> R + Sync,
          R: Send
{
    type Item = R;

    fn next(&mut self) -> Option<R> {
        self.base.next().map(|item| (self.map_op)(&mut self.item, item))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
}

impl<'f, I, U, F, R> DoubleEndedIterator for MapWithIter<'f, I, U, F>
    where I: DoubleEndedIterator,
          U: Send + Clone,
          F: Fn(&mut U, I::Item) -> R + Sync,
          R: Send
{
    fn next_back(&mut self) -> Option<R> {
        self.base.next_back().map(|item| (self.map_op)(&mut self.item, item))
    }
}

impl<'f, I, U, F, R> ExactSizeIterator for MapWithIter<'f, I, U, F>
    where I: ExactSizeIterator,
          U: Send + Clone,
          F: Fn(&mut U, I::Item) -> R + Sync,
          R: Send
{
}


/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct MapWithConsumer<'f, C, U, F: 'f> {
    base: C,
    item: U,
    map_op: &'f F,
}

impl<'f, C, U, F> MapWithConsumer<'f, C, U, F> {
    fn new(base: C, item: U, map_op: &'f F) -> Self {
        MapWithConsumer {
            base: base,
            item: item,
            map_op: map_op,
        }
    }
}

impl<'f, T, U, R, C, F> Consumer<T> for MapWithConsumer<'f, C, U, F>
    where C: Consumer<R>,
          U: Send + Clone,
          F: Fn(&mut U, T) -> R + Sync,
          R: Send
{
    type Folder = MapWithFolder<'f, C::Folder, U, F>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (MapWithConsumer::new(left, self.item.clone(), self.map_op),
         MapWithConsumer::new(right, self.item, self.map_op),
         reducer)
    }

    fn into_folder(self) -> Self::Folder {
        MapWithFolder {
            base: self.base.into_folder(),
            item: self.item,
            map_op: self.map_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'f, T, U, R, C, F> UnindexedConsumer<T> for MapWithConsumer<'f, C, U, F>
    where C: UnindexedConsumer<R>,
          U: Send + Clone,
          F: Fn(&mut U, T) -> R + Sync,
          R: Send
{
    fn split_off_left(&self) -> Self {
        MapWithConsumer::new(self.base.split_off_left(), self.item.clone(), self.map_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct MapWithFolder<'f, C, U, F: 'f> {
    base: C,
    item: U,
    map_op: &'f F,
}

impl<'f, T, U, R, C, F> Folder<T> for MapWithFolder<'f, C, U, F>
    where C: Folder<R>,
          U: Clone,
          F: Fn(&mut U, T) -> R
{
    type Result = C::Result;

    fn consume(mut self, item: T) -> Self {
        let mapped_item = (self.map_op)(&mut self.item, item);
        self.base = self.base.consume(mapped_item);
        self
    }

    fn consume_iter<I>(mut self, iter: I) -> Self
        where I: IntoIterator<Item = T>
    {
        {
            let map_op = self.map_op;
            let item = &mut self.item;
            let mapped_iter = iter.into_iter().map(|x| map_op(item, x));
            self.base = self.base.consume_iter(mapped_iter);
        }
        self
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
