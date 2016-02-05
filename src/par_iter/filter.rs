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

    fn drive_unindexed<'c, C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<'c, Item=Self::Item>
    {
        let consumer1 = FilterConsumer::new(consumer, &self.filter_op);
        self.base.drive_unindexed(consumer1)
    }
}

unsafe impl<M, FILTER_OP> BoundedParallelIterator for Filter<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<'c, C>(self, consumer: C) -> C::Result
        where C: Consumer<'c, Item=Self::Item>
    {
        let consumer1 = FilterConsumer::new(consumer, &self.filter_op);
        self.base.drive(consumer1)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterConsumer<'f, 'c: 'f, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(&C::Item) -> bool + Sync + 'f
{
    base: C,
    filter_op: &'f FILTER_OP,
    phantoms: PhantomType<&'c ()>,
}

impl<'f, 'c, C, FILTER_OP> FilterConsumer<'f, 'c, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(&C::Item) -> bool + Sync
{
    fn new(base: C, filter_op: &'f FILTER_OP) -> Self {
        FilterConsumer { base: base, filter_op: filter_op, phantoms: PhantomType::new() }
    }
}

impl<'f, 'c, C, FILTER_OP: 'f> Consumer<'f> for FilterConsumer<'f, 'c, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(&C::Item) -> bool + Sync,
{
    type Item = C::Item;
    type SeqState = C::SeqState;
    type Result = C::Result;

    /// Cost to process `items` number of items.
    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (FilterConsumer::new(left, self.filter_op),
         FilterConsumer::new(right, self.filter_op))
    }

    unsafe fn start(&mut self) -> C::SeqState {
        self.base.start()
    }

    unsafe fn consume(&mut self,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        if (self.filter_op)(&item) {
            self.base.consume(state, item)
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

impl<'f, 'c, C, FILTER_OP: 'f> UnindexedConsumer<'f>
    for FilterConsumer<'f, 'c, C, FILTER_OP>
    where C: UnindexedConsumer<'c>, FILTER_OP: Fn(&C::Item) -> bool + Sync, 'c: 'f,
{
    fn split(&self) -> Self {
        FilterConsumer::new(self.base.split(), &self.filter_op)
    }
}

