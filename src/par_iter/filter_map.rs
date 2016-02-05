use super::*;
use super::len::*;
use super::internal::*;
use super::util::PhantomType;

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
        where C: UnindexedConsumer<Item=Self::Item>
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
        where C: Consumer<Item=Self::Item>
    {
        let consumer = FilterMapConsumer::new(consumer, &self.filter_op);
        self.base.drive(consumer)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterMapConsumer<'f, ITEM, C, FILTER_OP>
    where C: Consumer,
          FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync + 'f,
{
    base: C,
    filter_op: &'f FILTER_OP,
    phantoms: PhantomType<ITEM>,
}

impl<'f, ITEM, C, FILTER_OP> FilterMapConsumer<'f, ITEM, C, FILTER_OP>
    where C: Consumer,
          FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync,
{
    fn new(base: C, filter_op: &'f FILTER_OP) -> Self {
        FilterMapConsumer { base: base,
                            filter_op: filter_op,
                            phantoms: PhantomType::new() }
    }
}

impl<'f, ITEM, C, FILTER_OP> Consumer for FilterMapConsumer<'f, ITEM, C, FILTER_OP>
    where C: Consumer,
          FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync,
          FILTER_OP: 'f
{
    type Item = ITEM;
    type Folder = FilterMapFolder<'f, ITEM, C::Folder, FILTER_OP>;
    type Reducer = C::Reducer;
    type Result = C::Result;

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

    fn fold(self) -> Self::Folder {
        let base = self.base.fold();
        FilterMapFolder { base: base,
                          filter_op: self.filter_op,
                          phantoms: self.phantoms }
    }
}

impl<'f, ITEM, C, FILTER_OP> UnindexedConsumer
    for FilterMapConsumer<'f, ITEM, C, FILTER_OP>
    where C: UnindexedConsumer,
          FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync,
{
    fn split(&self) -> Self {
        FilterMapConsumer::new(self.base.split(), &self.filter_op)
    }

    fn reducer(&self) -> Self::Reducer {
        self.base.reducer()
    }
}

struct FilterMapFolder<'f, ITEM, C, FILTER_OP>
    where C: Folder,
          FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync + 'f,
{
    base: C,
    filter_op: &'f FILTER_OP,
    phantoms: PhantomType<ITEM>,
}

impl<'f, ITEM, C, FILTER_OP> Folder for FilterMapFolder<'f, ITEM, C, FILTER_OP>
    where C: Folder,
          FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync + 'f,
{
    type Item = ITEM;
    type Result = C::Result;

    fn consume(self, item: Self::Item) -> Self {
        let filter_op = self.filter_op;
        if let Some(mapped_item) = filter_op(item) {
            let base = self.base.consume(mapped_item);
            FilterMapFolder { base: base,
                              filter_op: filter_op,
                              phantoms: PhantomType::new() }
        } else {
            self
        }
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }
}

