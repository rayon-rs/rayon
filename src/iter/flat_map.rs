use super::internal::*;
use super::*;
use std::f64;

/// `FlatMap` maps each element to an iterator, then flattens these iterators together.
/// This struct is created by the [`flat_map()`] method on [`ParallelIterator`]
///
/// [`flap_map()`]: trait.ParallelIterator.html#method.flat_map
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct FlatMap<M: ParallelIterator, F> {
    base: M,
    map_op: F,
}

/// Create a new `FlatMap` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<M, F>(base: M, map_op: F) -> FlatMap<M, F>
    where M: ParallelIterator
{
    FlatMap {
        base: base,
        map_op: map_op,
    }
}

impl<M, F, PI> ParallelIterator for FlatMap<M, F>
    where M: ParallelIterator,
          F: Fn(M::Item) -> PI + Sync,
          PI: IntoParallelIterator
{
    type Item = PI::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer = FlatMapConsumer {
            base: consumer,
            map_op: &self.map_op,
        };
        self.base.drive_unindexed(consumer)
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct FlatMapConsumer<'m, C, F: 'm> {
    base: C,
    map_op: &'m F,
}

impl<'m, C, F> FlatMapConsumer<'m, C, F> {
    fn new(base: C, map_op: &'m F) -> Self {
        FlatMapConsumer {
            base: base,
            map_op: map_op,
        }
    }
}

impl<'m, I, MAPPED_ITEM, C, F> Consumer<I> for FlatMapConsumer<'m, C, F>
    where C: UnindexedConsumer<MAPPED_ITEM::Item>,
          F: Fn(I) -> MAPPED_ITEM + Sync,
          MAPPED_ITEM: IntoParallelIterator
{
    type Folder = FlatMapFolder<'m, C, F, C::Result>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn weighted(&self) -> bool {
        true
    }

    fn cost(&mut self, _cost: f64) -> f64 {
        // We have no idea how many items we will produce, so ramp up
        // the cost, so as to encourage the producer to do a
        // fine-grained divison. This is not necessarily a good
        // policy.
        f64::INFINITY
    }

    fn split_at(self, _index: usize) -> (Self, Self, C::Reducer) {
        (FlatMapConsumer::new(self.base.split_off_left(), self.map_op),
         FlatMapConsumer::new(self.base.split_off_left(), self.map_op),
         self.base.to_reducer())
    }

    fn into_folder(self) -> Self::Folder {
        FlatMapFolder {
            base: self.base,
            map_op: self.map_op,
            previous: None,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'m, I, MAPPED_ITEM, C, F> UnindexedConsumer<I> for FlatMapConsumer<'m, C, F>
    where C: UnindexedConsumer<MAPPED_ITEM::Item>,
          F: Fn(I) -> MAPPED_ITEM + Sync,
          MAPPED_ITEM: IntoParallelIterator
{
    fn split_off_left(&self) -> Self {
        FlatMapConsumer::new(self.base.split_off_left(), self.map_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}


struct FlatMapFolder<'m, C, F: 'm, R> {
    base: C,
    map_op: &'m F,
    previous: Option<R>,
}

impl<'m, I, MAPPED_ITEM, C, F> Folder<I> for FlatMapFolder<'m, C, F, C::Result>
    where C: UnindexedConsumer<MAPPED_ITEM::Item>,
          F: Fn(I) -> MAPPED_ITEM + Sync,
          MAPPED_ITEM: IntoParallelIterator
{
    type Result = C::Result;

    fn consume(self, item: I) -> Self {
        let map_op = self.map_op;
        let par_iter = map_op(item).into_par_iter();
        let result = par_iter.drive_unindexed(self.base.split_off_left());

        // We expect that `previous` is `None`, because we drive
        // the cost up so high, but just in case.
        let previous = match self.previous {
            None => Some(result),
            Some(previous) => {
                let reducer = self.base.to_reducer();
                Some(reducer.reduce(previous, result))
            }
        };

        FlatMapFolder {
            base: self.base,
            map_op: map_op,
            previous: previous,
        }
    }

    fn complete(self) -> Self::Result {
        match self.previous {
            Some(previous) => previous,
            None => self.base.into_folder().complete(),
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

