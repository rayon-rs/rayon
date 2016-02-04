use super::*;
use super::len::*;
use super::internal::*;
use super::util::PhantomType;

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

    fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result {
        let consumer1: MapConsumer<M::Item, C, MAP_OP> = MapConsumer::new(consumer);
        let shared1 = (shared, &self.map_op);
        self.base.drive_unindexed(consumer1, &shared1)
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

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        let consumer1: MapConsumer<M::Item, C, MAP_OP> = MapConsumer::new(consumer);
        let shared1 = (shared, &self.map_op);
        self.base.drive(consumer1, &shared1)
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

            fn callback<'p, P>(self,
                               base: P,
                               shared: &'p P::Shared)
                               -> CB::Output
                where P: Producer<'p, Item=ITEM>
            {
                let producer = MapProducer { base: base, phantoms: PhantomType::new() };
                self.callback.callback(producer, &(shared, &self.map_op))
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct MapProducer<'p, P, MAP_OP, R>
    where P: Producer<'p>,
          MAP_OP: Fn(P::Item) -> R + Sync,
          R: Send,
{
    base: P,
    phantoms: PhantomType<(&'p (), R, MAP_OP)>,
}

impl<'m, 'p, P, MAP_OP, R> Producer<'m> for MapProducer<'p, P, MAP_OP, R>
    where P: Producer<'p>,
          MAP_OP: Fn(P::Item) -> R + Sync + 'm,
          R: Send,
          'p: 'm
{
    type Item = R;
    type Shared = (&'p P::Shared, &'m MAP_OP);

    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64 {
        self.base.cost(&shared.0, len) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapProducer { base: left, phantoms: PhantomType::new() },
         MapProducer { base: right, phantoms: PhantomType::new() })
    }

    fn produce(&mut self, shared: &Self::Shared) -> R {
        let item = self.base.produce(&shared.0);
        (shared.1)(item)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct MapConsumer<'c, ITEM, C, MAP_OP>
    where C: Consumer<'c>, MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    base: C,
    phantoms: PhantomType<(&'c (), ITEM, MAP_OP)>,
}

impl<'c, ITEM, C, MAP_OP> MapConsumer<'c, ITEM, C, MAP_OP>
    where C: Consumer<'c>, MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    fn new(base: C) -> MapConsumer<'c, ITEM, C, MAP_OP> {
        MapConsumer { base: base, phantoms: PhantomType::new() }
    }
}

impl<'m, 'c, ITEM, C, MAP_OP> Consumer<'m> for MapConsumer<'c, ITEM, C, MAP_OP>
    where C: Consumer<'c>,
          MAP_OP: Fn(ITEM) -> C::Item + Sync,
          ITEM: 'm,
          MAP_OP: 'm,
          'c: 'm,
{
    type Item = ITEM;
    type Shared = (&'c C::Shared, &'m MAP_OP);
    type SeqState = C::SeqState;
    type Result = C::Result;

    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        self.base.cost(&shared.0, cost) * FUNC_ADJUSTMENT
    }

    fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.0, index);
        (MapConsumer::new(left), MapConsumer::new(right))
    }

    fn start(&mut self, shared: &Self::Shared) -> C::SeqState {
        self.base.start(&shared.0)
    }

    fn consume(&mut self,
                      shared: &Self::Shared,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        let mapped_item = (shared.1)(item);
        self.base.consume(&shared.0, state, mapped_item)
    }

    fn complete(self, shared: &Self::Shared, state: C::SeqState) -> C::Result {
        self.base.complete(&shared.0, state)
    }

    fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
        C::reduce(&shared.0, left, right)
    }
}

impl<'m, 'c, ITEM, C, MAP_OP> UnindexedConsumer<'m> for MapConsumer<'c, ITEM, C, MAP_OP>
    where C: UnindexedConsumer<'c>,
          MAP_OP: Fn(ITEM) -> C::Item + Sync,
          ITEM: 'm,
          MAP_OP: 'm,
          'c: 'm,
{
    fn split(&self) -> Self {
        MapConsumer::new(self.base.split())
    }
}
