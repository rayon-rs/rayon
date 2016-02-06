use super::*;
use super::len::*;
use super::internal::*;
use std::iter;

pub struct Map<M, MAP_OP> {
    base: M,
    map_op: MAP_OP,
}

impl<M, MAP_OP> Map<M, MAP_OP> {
    pub fn new(base: M, map_op: MAP_OP) -> Map<M, MAP_OP> {
        Map { base: base, map_op: map_op }
    }
}

impl<M, MAP_OP, R> ParallelIterator for Map<M, MAP_OP>
    where M: ParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
{
    type Item = R;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = MapConsumer::new(consumer, &self.map_op);
        self.base.drive_unindexed(consumer1)
    }
}

impl<M, MAP_OP, R> BoundedParallelIterator for Map<M, MAP_OP>
    where M: BoundedParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        let consumer1 = MapConsumer::new(consumer, &self.map_op);
        self.base.drive(consumer1)
    }
}

impl<M, MAP_OP, R> ExactParallelIterator for Map<M, MAP_OP>
    where M: ExactParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M, MAP_OP, R> IndexedParallelIterator for Map<M, MAP_OP>
    where M: IndexedParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback { callback: callback, map_op: self.map_op });

        struct Callback<CB, MAP_OP> {
            callback: CB,
            map_op: MAP_OP,
        }

        impl<ITEM, R, MAP_OP, CB> ProducerCallback<ITEM> for Callback<CB, MAP_OP>
            where MAP_OP: Fn(ITEM) -> R + Sync,
                  R: Send,
                  CB: ProducerCallback<R>
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item=ITEM>
            {
                let producer = MapProducer { base: base,
                                             map_op: &self.map_op };
                self.callback.callback(producer)
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct MapProducer<'m, P, MAP_OP: 'm> {
    base: P,
    map_op: &'m MAP_OP,
}

impl<'m, P, MAP_OP, RET> Producer for MapProducer<'m, P, MAP_OP>
    where P: Producer,
          MAP_OP: Fn(P::Item) -> RET + Sync,
          RET: Send,
{
    fn cost(&mut self, len: usize) -> f64 {
        self.base.cost(len) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapProducer { base: left, map_op: self.map_op, },
         MapProducer { base: right, map_op: self.map_op, })
    }
}

impl<'m, P, MAP_OP, RET> IntoIterator for MapProducer<'m, P, MAP_OP>
    where P: Producer,
          MAP_OP: Fn(P::Item) -> RET + Sync,
          RET: Send,
{
    type Item = RET;
    type IntoIter = iter::Map<P::IntoIter, &'m MAP_OP>;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter().map(self.map_op)
    }
}


///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct MapConsumer<'m, C, MAP_OP: 'm> {
    base: C,
    map_op: &'m MAP_OP,
}

impl<'m, C, MAP_OP> MapConsumer<'m, C, MAP_OP>
{
    fn new(base: C, map_op: &'m MAP_OP) -> Self {
        MapConsumer { base: base, map_op: map_op, }
    }
}

impl<'m, ITEM, MAPPED_ITEM, C, MAP_OP> Consumer<ITEM>
    for MapConsumer<'m, C, MAP_OP>
    where C: Consumer<MAPPED_ITEM>,
          MAP_OP: Fn(ITEM) -> MAPPED_ITEM + Sync,
{
    type Folder = MapFolder<'m, C::Folder, MAP_OP>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (MapConsumer::new(left, self.map_op),
         MapConsumer::new(right, self.map_op),
         reducer)
    }

    fn fold(self) -> Self::Folder {
        MapFolder {
            base: self.base.fold(),
            map_op: self.map_op,
        }
    }
}

impl<'m, ITEM, MAPPED_ITEM, C, MAP_OP> UnindexedConsumer<ITEM>
    for MapConsumer<'m, C, MAP_OP>
    where C: UnindexedConsumer<MAPPED_ITEM>,
          MAP_OP: Fn(ITEM) -> MAPPED_ITEM + Sync,
{
    fn split(&self) -> Self {
        MapConsumer::new(self.base.split(), &self.map_op)
    }

    fn reducer(&self) -> Self::Reducer {
        self.base.reducer()
    }
}

struct MapFolder<'m, C, MAP_OP: 'm>
{
    base: C,
    map_op: &'m MAP_OP,
}

impl<'m, ITEM, MAPPED_ITEM, C, MAP_OP> Folder<ITEM> for MapFolder<'m, C, MAP_OP>
    where C: Folder<MAPPED_ITEM>,
          MAP_OP: Fn(ITEM) -> MAPPED_ITEM + Sync,
{
    type Result = C::Result;

    fn consume(self, item: ITEM) -> Self {
        let map_op = self.map_op;
        let mapped_item = map_op(item);
        let base = self.base.consume(mapped_item);
        MapFolder { base: base, map_op: map_op }
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }
}

