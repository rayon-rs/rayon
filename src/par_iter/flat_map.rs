use super::*;
use super::internal::*;
use super::util::PhantomType;
use std::f64;

pub struct FlatMap<M, MAP_OP> {
    base: M,
    map_op: MAP_OP,
}

impl<M, MAP_OP> FlatMap<M, MAP_OP> {
    pub fn new(base: M, map_op: MAP_OP) -> FlatMap<M, MAP_OP> {
        FlatMap { base: base, map_op: map_op }
    }
}

impl<M, MAP_OP, PI> ParallelIterator for FlatMap<M, MAP_OP>
    where M: ParallelIterator,
          MAP_OP: Fn(M::Item) -> PI + Sync,
          PI: ParallelIterator,
{
    type Item = PI::Item;

    fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result {
        let consumer = FlatMapConsumer { base: consumer, phantoms: PhantomType::new() };
        self.base.drive_unindexed(consumer, &(shared, &self.map_op))
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FlatMapConsumer<'c, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer<'c>,
          MAP_OP: Fn(ITEM) -> PI + Sync,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
{
    base: C,
    phantoms: PhantomType<(&'c (), ITEM, MAP_OP)>,
}

impl<'c, ITEM, C, MAP_OP, PI> FlatMapConsumer<'c, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer<'c>,
          MAP_OP: Fn(ITEM) -> PI + Sync,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
{
    fn new(base: C) -> FlatMapConsumer<'c, ITEM, C, MAP_OP, PI> {
        FlatMapConsumer { base: base, phantoms: PhantomType::new() }
    }
}

impl<'c, 'm, ITEM, C, MAP_OP, PI> Consumer<'m> for FlatMapConsumer<'c, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer<'c>,
          MAP_OP: Fn(ITEM) -> PI + Sync + 'm,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
          ITEM: 'm,
          'c: 'm,
{
    type Item = ITEM;
    type Shared = (&'c C::Shared, &'m MAP_OP);
    type SeqState = Option<C::Result>;
    type Result = C::Result;

    fn cost(&mut self, _shared: &Self::Shared, _cost: f64) -> f64 {
        // We have no idea how many items we will produce, so ramp up
        // the cost, so as to encourage the producer to do a
        // fine-grained divison. This is not necessarily a good
        // policy.
        f64::INFINITY
    }

    unsafe fn split_at(self, _shared: &Self::Shared, _index: usize) -> (Self, Self) {
        (FlatMapConsumer::new(self.base.split()), FlatMapConsumer::new(self.base.split()))
    }

    unsafe fn start(&mut self, _shared: &Self::Shared) -> Option<C::Result> {
        None
    }

    unsafe fn consume(&mut self,
                      shared: &Self::Shared,
                      previous: Option<C::Result>,
                      item: Self::Item)
                      -> Option<C::Result>
    {
        let par_iter = (shared.1)(item).into_par_iter();
        let result = par_iter.drive_unindexed(self.base.split(), &shared.0);

        // We expect that `previous` is `None`, because we drive
        // the cost up so high, but just in case.
        match previous {
            Some(previous) => Some(C::reduce(&shared.0, result, previous)),
            None => Some(result),
        }
    }

    unsafe fn complete(self, _shared: &Self::Shared, state: Option<C::Result>) -> C::Result {
        // should have processed at least one item -- but is this
        // really a fair assumption?
        state.unwrap()
    }

    unsafe fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
        C::reduce(&shared.0, left, right)
    }
}

impl<'c, 'm, ITEM, C, MAP_OP, PI> UnindexedConsumer<'m> for FlatMapConsumer<'c, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer<'c>,
          MAP_OP: Fn(ITEM) -> PI + Sync + 'm,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
          ITEM: 'm,
          'c: 'm,
{
    fn split(&self) -> Self {
        FlatMapConsumer::new(self.base.split())
    }
}

