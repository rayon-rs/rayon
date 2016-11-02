use super::*;
use super::internal::*;
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
          PI: IntoParallelIterator,
{
    type Item = PI::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer = FlatMapConsumer { base: consumer,
                                         map_op: &self.map_op };
        self.base.drive_unindexed(consumer)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FlatMapConsumer<'m, C, MAP_OP: 'm> {
    base: C,
    map_op: &'m MAP_OP,
}

impl<'m, C, MAP_OP> FlatMapConsumer<'m, C, MAP_OP> {
    fn new(base: C, map_op: &'m MAP_OP) -> Self {
        FlatMapConsumer { base: base, map_op: map_op }
    }
}

impl<'m, ITEM, MAPPED_ITEM, C, MAP_OP> Consumer<ITEM>
    for FlatMapConsumer<'m, C, MAP_OP>
    where C: UnindexedConsumer<MAPPED_ITEM::Item>,
          MAP_OP: Fn(ITEM) -> MAPPED_ITEM + Sync,
          MAPPED_ITEM: IntoParallelIterator,
{
    type Folder = FlatMapFolder<'m, C, MAP_OP, C::Result>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn weighted(&self) -> bool {
        true
    }

    fn cost(&mut self, _cost: f64) -> f64 {
        // We have no idea how many items we will produce, so ramp up
        // the cost, so as to encourage the producer to do a
        // fine-grained divison. This is not necessarily a good
        // policy.
        f64::INFINITY
    }

    fn split_at(self, _index: usize) -> (Self, Self, C::Reducer) {
        (FlatMapConsumer::new(self.base.split_off(), self.map_op),
         FlatMapConsumer::new(self.base.split_off(), self.map_op),
         self.base.to_reducer())
    }

    fn into_folder(self) -> Self::Folder {
        FlatMapFolder {
            base: self.base,
            map_op: self.map_op,
            previous: None,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'m, ITEM, MAPPED_ITEM, C, MAP_OP> UnindexedConsumer<ITEM>
    for FlatMapConsumer<'m, C, MAP_OP>
    where C: UnindexedConsumer<MAPPED_ITEM::Item>,
          MAP_OP: Fn(ITEM) -> MAPPED_ITEM + Sync,
          MAPPED_ITEM: IntoParallelIterator,
{
    fn split_off(&self) -> Self {
        FlatMapConsumer::new(self.base.split_off(), self.map_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}


struct FlatMapFolder<'m, C, MAP_OP: 'm, R> {
    base: C,
    map_op: &'m MAP_OP,
    previous: Option<R>,
}

impl<'m, ITEM, MAPPED_ITEM, C, MAP_OP> Folder<ITEM>
    for FlatMapFolder<'m, C, MAP_OP, C::Result>
    where C: UnindexedConsumer<MAPPED_ITEM::Item>,
          MAP_OP: Fn(ITEM) -> MAPPED_ITEM + Sync,
          MAPPED_ITEM: IntoParallelIterator,
{
    type Result = C::Result;

    fn consume(self, item: ITEM) -> Self {
        let map_op = self.map_op;
        let par_iter = map_op(item).into_par_iter();
        let result = par_iter.drive_unindexed(self.base.split_off());

        // We expect that `previous` is `None`, because we drive
        // the cost up so high, but just in case.
        let previous = match self.previous {
            None => Some(result),
            Some(previous) => {
                let reducer = self.base.to_reducer();
                Some(reducer.reduce(result, previous))
            }
        };

        FlatMapFolder { base: self.base,
                        map_op: map_op,
                        previous: previous }
    }

    fn complete(self) -> Self::Result {
        match self.previous {
            Some(previous) => previous,
            None => self.base.into_folder().complete(),
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
