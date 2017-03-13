use super::internal::*;
use super::len::*;
use super::*;

/// `FilterMap` creates an iterator that uses `filter_op` to both filter and map elements.
/// This struct is created by the [`filter_map()`] method on [`ParallelIterator`].
///
/// [`filter_map()`]: trait.ParallelIterator.html#method.filter_map
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct FilterMap<M: ParallelIterator, P> {
    base: M,
    filter_op: P,
}

/// Create a new `FilterMap` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<M, P>(base: M, filter_op: P) -> FilterMap<M, P>
    where M: ParallelIterator
{
    FilterMap {
        base: base,
        filter_op: filter_op,
    }
}

impl<M, P, R> ParallelIterator for FilterMap<M, P>
    where M: ParallelIterator,
          P: Fn(M::Item) -> Option<R> + Sync,
          R: Send
{
    type Item = R;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer = FilterMapConsumer::new(consumer, &self.filter_op);
        self.base.drive_unindexed(consumer)
    }
}

impl<M, P, R> BoundedParallelIterator for FilterMap<M, P>
    where M: BoundedParallelIterator,
          P: Fn(M::Item) -> Option<R> + Sync,
          R: Send
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        let consumer = FilterMapConsumer::new(consumer, &self.filter_op);
        self.base.drive(consumer)
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct FilterMapConsumer<'p, C, P: 'p> {
    base: C,
    filter_op: &'p P,
}

impl<'p, C, P: 'p> FilterMapConsumer<'p, C, P> {
    fn new(base: C, filter_op: &'p P) -> Self {
        FilterMapConsumer {
            base: base,
            filter_op: filter_op,
        }
    }
}

impl<'p, ITEM, MAPPED_ITEM, C, P> Consumer<ITEM> for FilterMapConsumer<'p, C, P>
    where C: Consumer<MAPPED_ITEM>,
          P: Fn(ITEM) -> Option<MAPPED_ITEM> + Sync + 'p
{
    type Folder = FilterMapFolder<'p, C::Folder, P>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn weighted(&self) -> bool {
        self.base.weighted()
    }

    /// Cost to process `items` number of items.
    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (FilterMapConsumer::new(left, self.filter_op),
         FilterMapConsumer::new(right, self.filter_op),
         reducer)
    }

    fn into_folder(self) -> Self::Folder {
        let base = self.base.into_folder();
        FilterMapFolder {
            base: base,
            filter_op: self.filter_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'p, ITEM, MAPPED_ITEM, C, P> UnindexedConsumer<ITEM>
    for FilterMapConsumer<'p, C, P>
    where C: UnindexedConsumer<MAPPED_ITEM>,
          P: Fn(ITEM) -> Option<MAPPED_ITEM> + Sync + 'p
{
    fn split_off_left(&self) -> Self {
        FilterMapConsumer::new(self.base.split_off_left(), &self.filter_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct FilterMapFolder<'p, C, P: 'p> {
    base: C,
    filter_op: &'p P,
}

impl<'p, I, C_ITEM, C, P> Folder<I> for FilterMapFolder<'p, C, P>
    where C: Folder<C_ITEM>,
          P: Fn(I) -> Option<C_ITEM> + Sync + 'p
{
    type Result = C::Result;

    fn consume(self, item: I) -> Self {
        let filter_op = self.filter_op;
        if let Some(mapped_item) = filter_op(item) {
            let base = self.base.consume(mapped_item);
            FilterMapFolder {
                base: base,
                filter_op: filter_op,
            }
        } else {
            self
        }
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
