use super::internal::*;
use super::len::*;
use super::*;

/// `Filter` takes a predicate `filter_op` and filters out elements that match.
/// This struct is created by the [`filter()`] method on [`ParallelIterator`]
///
/// [`filter()`]: trait.ParallelIterator.html#method.filter
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct Filter<M: ParallelIterator, P> {
    base: M,
    filter_op: P,
}

/// Create a new `Filter` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<M, P>(base: M, filter_op: P) -> Filter<M, P>
    where M: ParallelIterator
{
    Filter {
        base: base,
        filter_op: filter_op,
    }
}

impl<M, P> ParallelIterator for Filter<M, P>
    where M: ParallelIterator,
          P: Fn(&M::Item) -> bool + Sync
{
    type Item = M::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = FilterConsumer::new(consumer, &self.filter_op);
        self.base.drive_unindexed(consumer1)
    }
}

impl<M, P> BoundedParallelIterator for Filter<M, P>
    where M: BoundedParallelIterator,
          P: Fn(&M::Item) -> bool + Sync
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        let consumer1 = FilterConsumer::new(consumer, &self.filter_op);
        self.base.drive(consumer1)
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct FilterConsumer<'p, C, P: 'p> {
    base: C,
    filter_op: &'p P,
}

impl<'p, C, P> FilterConsumer<'p, C, P> {
    fn new(base: C, filter_op: &'p P) -> Self {
        FilterConsumer {
            base: base,
            filter_op: filter_op,
        }
    }
}

impl<'p, I, C, P: 'p> Consumer<I> for FilterConsumer<'p, C, P>
    where C: Consumer<I>,
          P: Fn(&I) -> bool + Sync
{
    type Folder = FilterFolder<'p, C::Folder, P>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn weighted(&self) -> bool {
        self.base.weighted()
    }

    /// Cost to process `items` number of items.
    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self, C::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (FilterConsumer::new(left, self.filter_op),
         FilterConsumer::new(right, self.filter_op),
         reducer)
    }

    fn into_folder(self) -> Self::Folder {
        FilterFolder {
            base: self.base.into_folder(),
            filter_op: self.filter_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}


impl<'p, I, C, P: 'p> UnindexedConsumer<I> for FilterConsumer<'p, C, P>
    where C: UnindexedConsumer<I>,
          P: Fn(&I) -> bool + Sync
{
    fn split_off_left(&self) -> Self {
        FilterConsumer::new(self.base.split_off_left(), &self.filter_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct FilterFolder<'p, C, P: 'p> {
    base: C,
    filter_op: &'p P,
}

impl<'p, C, P, I> Folder<I> for FilterFolder<'p, C, P>
    where C: Folder<I>,
          P: Fn(&I) -> bool + 'p
{
    type Result = C::Result;

    fn consume(self, item: I) -> Self {
        let filter_op = self.filter_op;
        if filter_op(&item) {
            let base = self.base.consume(item);
            FilterFolder {
                base: base,
                filter_op: filter_op,
            }
        } else {
            self
        }
    }

    fn complete(self) -> Self::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
