use super::internal::*;
use super::len::*;
use super::*;

pub struct Filter<M, FILTER_OP> {
    base: M,
    filter_op: FILTER_OP,
}

impl<M, FILTER_OP> Filter<M, FILTER_OP> {
    pub fn new(base: M, filter_op: FILTER_OP) -> Filter<M, FILTER_OP> {
        Filter {
            base: base,
            filter_op: filter_op,
        }
    }
}

impl<M, FILTER_OP> ParallelIterator for Filter<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync
{
    type Item = M::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = FilterConsumer::new(consumer, &self.filter_op);
        self.base.drive_unindexed(consumer1)
    }
}

impl<M, FILTER_OP> BoundedParallelIterator for Filter<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync
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

struct FilterConsumer<'f, C, FILTER_OP: 'f> {
    base: C,
    filter_op: &'f FILTER_OP,
}

impl<'f, C, FILTER_OP> FilterConsumer<'f, C, FILTER_OP> {
    fn new(base: C, filter_op: &'f FILTER_OP) -> Self {
        FilterConsumer {
            base: base,
            filter_op: filter_op,
        }
    }
}

impl<'f, ITEM, C, FILTER_OP: 'f> Consumer<ITEM> for FilterConsumer<'f, C, FILTER_OP>
    where C: Consumer<ITEM>,
          FILTER_OP: Fn(&ITEM) -> bool + Sync
{
    type Folder = FilterFolder<'f, C::Folder, FILTER_OP>;
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


impl<'f, ITEM, C, FILTER_OP: 'f> UnindexedConsumer<ITEM> for FilterConsumer<'f, C, FILTER_OP>
    where C: UnindexedConsumer<ITEM>,
          FILTER_OP: Fn(&ITEM) -> bool + Sync
{
    fn split_off(&self) -> Self {
        FilterConsumer::new(self.base.split_off(), &self.filter_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct FilterFolder<'f, C, FILTER_OP: 'f> {
    base: C,
    filter_op: &'f FILTER_OP,
}

impl<'f, C, FILTER_OP, ITEM> Folder<ITEM> for FilterFolder<'f, C, FILTER_OP>
    where C: Folder<ITEM>,
          FILTER_OP: Fn(&ITEM) -> bool + 'f
{
    type Result = C::Result;

    fn consume(self, item: ITEM) -> Self {
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
