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

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Item=Self::Item>
    {
        let consumer = FlatMapConsumer { base: consumer,
                                         map_op: &self.map_op,
                                         phantoms: PhantomType::new() };
        self.base.drive_unindexed(consumer)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FlatMapConsumer<'m, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer,
          MAP_OP: Fn(ITEM) -> PI + Sync + 'm,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
          ITEM: 'm,
{
    base: C,
    map_op: &'m MAP_OP,
    phantoms: PhantomType<ITEM>,
}

impl<'m, ITEM, C, MAP_OP, PI> FlatMapConsumer<'m, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer,
          MAP_OP: Fn(ITEM) -> PI + Sync,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
{
    fn new(base: C, map_op: &'m MAP_OP) -> Self {
        FlatMapConsumer { base: base, map_op: map_op, phantoms: PhantomType::new() }
    }
}

impl<'m, ITEM, C, MAP_OP, PI> Consumer
    for FlatMapConsumer<'m, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer,
          MAP_OP: Fn(ITEM) -> PI + Sync,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
{
    type Item = ITEM;
    type SeqState = Option<C::Result>;
    type Result = C::Result;

    fn cost(&mut self, _cost: f64) -> f64 {
        // We have no idea how many items we will produce, so ramp up
        // the cost, so as to encourage the producer to do a
        // fine-grained divison. This is not necessarily a good
        // policy.
        f64::INFINITY
    }

    unsafe fn split_at(self, _index: usize) -> (Self, Self) {
        (FlatMapConsumer::new(self.base.split(), self.map_op),
         FlatMapConsumer::new(self.base.split(), self.map_op))
    }

    unsafe fn start(&mut self) -> Option<C::Result> {
        None
    }

    unsafe fn consume(&mut self,
                      previous: Option<C::Result>,
                      item: Self::Item)
                      -> Option<C::Result>
    {
        let par_iter = (self.map_op)(item).into_par_iter();
        let result = par_iter.drive_unindexed(self.base.split());

        // We expect that `previous` is `None`, because we drive
        // the cost up so high, but just in case.
        match previous {
            Some(previous) => Some(C::reduce(result, previous)),
            None => Some(result),
        }
    }

    unsafe fn complete(self, state: Option<C::Result>) -> C::Result {
        // should have processed at least one item -- but is this
        // really a fair assumption?
        state.unwrap()
    }

    unsafe fn reduce(left: C::Result, right: C::Result) -> C::Result {
        C::reduce(left, right)
    }
}

impl<'m, ITEM, C, MAP_OP, PI> UnindexedConsumer
    for FlatMapConsumer<'m, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer,
          MAP_OP: Fn(ITEM) -> PI + Sync + 'm,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
          ITEM: 'm,
{
    fn split(&self) -> Self {
        FlatMapConsumer::new(self.base.split(), self.map_op)
    }
}

