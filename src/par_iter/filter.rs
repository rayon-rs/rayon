use super::*;
use super::len::*;
use super::internal::*;

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

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Item=Self::Item>
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
        where C: Consumer<Item=Self::Item>
    {
        let consumer1 = FilterConsumer::new(consumer, &self.filter_op);
        self.base.drive(consumer1)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterConsumer<'f, C, FILTER_OP>
    where C: Consumer, FILTER_OP: Fn(&C::Item) -> bool + Sync + 'f
{
    base: C,
    filter_op: &'f FILTER_OP,
}

impl<'f, C, FILTER_OP> FilterConsumer<'f, C, FILTER_OP>
    where C: Consumer, FILTER_OP: Fn(&C::Item) -> bool + Sync
{
    fn new(base: C, filter_op: &'f FILTER_OP) -> Self {
        FilterConsumer { base: base, filter_op: filter_op }
    }
}

impl<'f, C, FILTER_OP: 'f> Consumer for FilterConsumer<'f, C, FILTER_OP>
    where C: Consumer, FILTER_OP: Fn(&C::Item) -> bool + Sync,
{
    type Item = C::Item;
    type SeqState = C::SeqState;
    type Result = C::Result;

    /// Cost to process `items` number of items.
    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (FilterConsumer::new(left, self.filter_op),
         FilterConsumer::new(right, self.filter_op))
    }

    fn start(&mut self) -> C::SeqState {
        self.base.start()
    }

    fn consume(&mut self, state: C::SeqState, item: Self::Item) -> C::SeqState {
        if (self.filter_op)(&item) {
            self.base.consume(state, item)
        } else {
            state
        }
    }

    fn complete(self, state: C::SeqState) -> C::Result {
        self.base.complete(state)
    }

    fn reduce(left: C::Result, right: C::Result) -> C::Result {
        C::reduce(left, right)
    }
}

impl<'f, C, FILTER_OP: 'f> UnindexedConsumer
    for FilterConsumer<'f, C, FILTER_OP>
    where C: UnindexedConsumer, FILTER_OP: Fn(&C::Item) -> bool + Sync,
{
    fn split(&self) -> Self {
        FilterConsumer::new(self.base.split(), &self.filter_op)
    }
}

