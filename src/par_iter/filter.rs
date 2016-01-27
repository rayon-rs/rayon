use super::*;
use super::len::*;
use super::internal::*;
use super::util::PhantomType;

pub struct Filter<M, FILTER_OP> {
    base: M,
    filter_op: FILTER_OP,
}

impl<M, FILTER_OP> Filter<M, FILTER_OP> {
    pub fn new(base: M, filter_op: FILTER_OP) -> Filter<M, FILTER_OP> {
        Filter { base: base, filter_op: filter_op }
    }
}

impl<M, FILTER_OP> ParallelIterator for Filter<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync,
{
    type Item = M::Item;

    fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result {
        let consumer1: FilterConsumer<C, FILTER_OP> = FilterConsumer::new(consumer);
        let shared1 = (shared, &self.filter_op);
        self.base.drive_unindexed(consumer1, &shared1)
    }
}

unsafe impl<M, FILTER_OP> BoundedParallelIterator for Filter<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        let consumer1: FilterConsumer<C, FILTER_OP> = FilterConsumer::new(consumer);
        let shared1 = (shared, &self.filter_op);
        self.base.drive(consumer1, &shared1)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterConsumer<'c, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(&C::Item) -> bool + Sync
{
    base: C,
    filter_op: PhantomType<(&'c (), FILTER_OP)>,
}

impl<'c, C, FILTER_OP> FilterConsumer<'c, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(&C::Item) -> bool + Sync
{
    fn new(base: C) -> FilterConsumer<'c, C, FILTER_OP> {
        FilterConsumer { base: base, filter_op: PhantomType::new() }
    }
}

impl<'f, 'c, C, FILTER_OP: 'f> Consumer<'f> for FilterConsumer<'c, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(&C::Item) -> bool + Sync, 'c: 'f,
{
    type Item = C::Item;
    type Shared = (&'c C::Shared, &'f FILTER_OP);
    type SeqState = C::SeqState;
    type Result = C::Result;

    /// Cost to process `items` number of items.
    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        self.base.cost(&shared.0, cost) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.0, index);
        (FilterConsumer::new(left), FilterConsumer::new(right))
    }

    unsafe fn start(&mut self, shared: &Self::Shared) -> C::SeqState {
        self.base.start(&shared.0)
    }

    unsafe fn consume(&mut self,
                      shared: &Self::Shared,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        if (shared.1)(&item) {
            self.base.consume(&shared.0, state, item)
        } else {
            state
        }
    }

    unsafe fn complete(self, shared: &Self::Shared, state: C::SeqState) -> C::Result {
        self.base.complete(&shared.0, state)
    }

    unsafe fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
        C::reduce(&shared.0, left, right)
    }
}

impl<'f, 'c, C, FILTER_OP: 'f> UnindexedConsumer<'f> for FilterConsumer<'c, C, FILTER_OP>
    where C: UnindexedConsumer<'c>, FILTER_OP: Fn(&C::Item) -> bool + Sync, 'c: 'f,
{
    fn split(&self) -> Self {
        FilterConsumer::new(self.base.split())
    }
}

