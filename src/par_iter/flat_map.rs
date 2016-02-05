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
    type Folder = FlatMapFolder<'m, ITEM, C, MAP_OP, PI>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn cost(&mut self, _cost: f64) -> f64 {
        // We have no idea how many items we will produce, so ramp up
        // the cost, so as to encourage the producer to do a
        // fine-grained divison. This is not necessarily a good
        // policy.
        f64::INFINITY
    }

    fn split_at(self, _index: usize) -> (Self, Self, C::Reducer) {
        (FlatMapConsumer::new(self.base.split(), self.map_op),
         FlatMapConsumer::new(self.base.split(), self.map_op),
         self.base.reducer())
    }

    fn fold(self) -> FlatMapFolder<'m, ITEM, C, MAP_OP, PI> {
        FlatMapFolder {
            base: self.base,
            map_op: self.map_op,
            previous: None,
            phantoms: self.phantoms
        }
    }
}

impl<'m, ITEM, C, MAP_OP, PI> UnindexedConsumer
    for FlatMapConsumer<'m, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer,
          MAP_OP: Fn(ITEM) -> PI + Sync + 'm,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
{
    fn split(&self) -> Self {
        FlatMapConsumer::new(self.base.split(), self.map_op)
    }

    fn reducer(&self) -> Self::Reducer {
        self.base.reducer()
    }
}


struct FlatMapFolder<'m, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer,
          MAP_OP: Fn(ITEM) -> PI + Sync + 'm,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
{
    base: C,
    map_op: &'m MAP_OP,
    previous: Option<C::Result>,
    phantoms: PhantomType<ITEM>,
}

impl<'m, ITEM, C, MAP_OP, PI> Folder for FlatMapFolder<'m, ITEM, C, MAP_OP, PI>
    where C: UnindexedConsumer,
          MAP_OP: Fn(ITEM) -> PI + Sync + 'm,
          PI: ParallelIterator<Item=C::Item>,
          C::Item: Send,
{
    type Item = ITEM;
    type Result = C::Result;

    fn consume(self, item: Self::Item) -> Self {
        let map_op = self.map_op;
        let par_iter = map_op(item).into_par_iter();
        let result = par_iter.drive_unindexed(self.base.split());

        // We expect that `previous` is `None`, because we drive
        // the cost up so high, but just in case.
        let previous = match self.previous {
            None => Some(result),
            Some(previous) => {
                let reducer = self.base.reducer();
                Some(reducer.reduce(result, previous))
            }
        };

        FlatMapFolder { base: self.base,
                        map_op: map_op,
                        previous: previous,
                        phantoms: self.phantoms }
    }

    fn complete(self) -> Self::Result {
        // should have processed at least one item -- but is this
        // really a fair assumption?
        self.previous.unwrap()
    }
}
