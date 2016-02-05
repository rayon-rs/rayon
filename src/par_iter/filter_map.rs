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

    fn drive_unindexed<'c, C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<'c, Item=Self::Item>
    {
        let consumer = FilterMapConsumer::new(consumer, &self.filter_op);
        self.base.drive_unindexed(consumer)
    }
}

unsafe impl<M, FILTER_OP, R> BoundedParallelIterator for FilterMap<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync,
          R: Send,
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self, consumer: C) -> C::Result {
        let consumer = FilterMapConsumer::new(consumer, &self.filter_op);
        self.base.drive(consumer)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterMapConsumer<'f, 'c: 'f, ITEM, C, FILTER_OP>
    where C: Consumer<'c>,
          FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync + 'f,
          ITEM: 'f,
{
    base: C,
    filter_op: &'f FILTER_OP,
    phantoms: PhantomType<(&'c (), ITEM)>,
}

impl<'f, 'c, ITEM, C, FILTER_OP> FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync,
{
    fn new(base: C, filter_op: &'f FILTER_OP) -> Self {
        FilterMapConsumer { base: base,
                            filter_op: filter_op,
                            phantoms: PhantomType::new() }
    }
}

impl<'f, 'c, ITEM, C, FILTER_OP> Consumer<'f> for FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync, ITEM: 'f, FILTER_OP: 'f
{
    type Item = ITEM;
    type SeqState = C::SeqState;
    type Result = C::Result;

    /// Cost to process `items` number of items.
    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (FilterMapConsumer::new(left, self.filter_op),
         FilterMapConsumer::new(right, self.filter_op))
    }

    unsafe fn start(&mut self) -> C::SeqState {
        self.base.start()
    }

    unsafe fn consume(&mut self,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        if let Some(mapped_item) = (self.filter_op)(item) {
            self.base.consume(state, mapped_item)
        } else {
            state
        }
    }

    unsafe fn complete(self, state: C::SeqState) -> C::Result {
        self.base.complete(state)
    }

    unsafe fn reduce(left: C::Result, right: C::Result) -> C::Result {
        C::reduce(left, right)
    }
}

impl<'f, 'c, ITEM, C, FILTER_OP> UnindexedConsumer<'f>
    for FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: UnindexedConsumer<'c>,
          FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync,
          ITEM: 'f
{
    fn split(&self) -> Self {
        FilterMapConsumer::new(self.base.split(), &self.filter_op)
    }
}

