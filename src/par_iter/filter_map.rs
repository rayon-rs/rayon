use super::*;
use super::len::*;
use super::internal::*;

pub struct FilterMap<M, FILTER_OP> {
    base: M,
    filter_op: FILTER_OP,
}

impl<M, FILTER_OP> FilterMap<M, FILTER_OP> {
    pub fn new(base: M, filter_op: FILTER_OP) -> FilterMap<M, FILTER_OP> {
        FilterMap { base: base, filter_op: filter_op }
    }
}

impl<M, FILTER_OP, R> ParallelIterator for FilterMap<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync,
          R: Send,
{
    type Item = R;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer = FilterMapConsumer::new(consumer, &self.filter_op);
        self.base.drive_unindexed(consumer)
    }
}

impl<M, FILTER_OP, R> BoundedParallelIterator for FilterMap<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync,
          R: Send,
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

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterMapConsumer<'f, C, FILTER_OP: 'f> {
    base: C,
    filter_op: &'f FILTER_OP,
}

impl<'f, C, FILTER_OP: 'f> FilterMapConsumer<'f, C, FILTER_OP> {
    fn new(base: C, filter_op: &'f FILTER_OP) -> Self {
        FilterMapConsumer { base: base,
                            filter_op: filter_op }
    }
}

impl<'f, ITEM, MAPPED_ITEM, C, FILTER_OP> Consumer<ITEM>
    for FilterMapConsumer<'f, C, FILTER_OP>
    where C: Consumer<MAPPED_ITEM>,
          FILTER_OP: Fn(ITEM) -> Option<MAPPED_ITEM> + Sync + 'f,
{
    type Folder = FilterMapFolder<'f, C::Folder, FILTER_OP>;
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
        FilterMapFolder { base: base,
                          filter_op: self.filter_op }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'f, ITEM, MAPPED_ITEM, C, FILTER_OP> UnindexedConsumer<ITEM>
    for FilterMapConsumer<'f, C, FILTER_OP>
    where C: UnindexedConsumer<MAPPED_ITEM>,
          FILTER_OP: Fn(ITEM) -> Option<MAPPED_ITEM> + Sync + 'f,
{
    fn split_off(&self) -> Self {
        FilterMapConsumer::new(self.base.split_off(), &self.filter_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct FilterMapFolder<'f, C, FILTER_OP: 'f> {
    base: C,
    filter_op: &'f FILTER_OP,
}

impl<'f, ITEM, C_ITEM, C, FILTER_OP> Folder<ITEM> for FilterMapFolder<'f, C, FILTER_OP>
    where C: Folder<C_ITEM>,
          FILTER_OP: Fn(ITEM) -> Option<C_ITEM> + Sync + 'f,
{
    type Result = C::Result;

    fn consume(self, item: ITEM) -> Self {
        let filter_op = self.filter_op;
        if let Some(mapped_item) = filter_op(item) {
            let base = self.base.consume(mapped_item);
            FilterMapFolder { base: base,
                              filter_op: filter_op }
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

