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

unsafe impl<M, MAP_OP, R> BoundedParallelIterator for Map<M, MAP_OP>
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

unsafe impl<M, MAP_OP, R> ExactParallelIterator for Map<M, MAP_OP>
    where M: ExactParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M, MAP_OP, R, P> IndexedParallelIterator for Map<M, MAP_OP>
    where M: IndexedParallelIterator<Producer=P>,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
          P: Producer<Item=M::Item>,
{
    type Producer = MapProducer<P, MAP_OP, R>;

    fn into_producer(self) -> (Self::Producer, (P::Shared, MAP_OP)) {
        let (base, shared) = self.base.into_producer();
        (MapProducer { base: base, phantoms: PhantomType::new() }, (shared, self.map_op))
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct MapProducer<M, MAP_OP, R>
    where M: Producer,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
{
    base: M,
    phantoms: PhantomType<MAP_OP>,
}

impl<M, MAP_OP, R> Producer for MapProducer<M, MAP_OP, R>
    where M: Producer,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
{
    type Item = R;
    type Shared = (M::Shared, MAP_OP);

    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64 {
        self.base.cost(&shared.0, len) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapProducer { base: left, phantoms: PhantomType::new() },
         MapProducer { base: right, phantoms: PhantomType::new() })
    }

    unsafe fn produce(&mut self, shared: &Self::Shared) -> R {
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

    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.0, index);
        (MapConsumer::new(left), MapConsumer::new(right))
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
        let mapped_item = (shared.1)(item);
        self.base.consume(&shared.0, state, mapped_item)
    }

    unsafe fn complete(self, shared: &Self::Shared, state: C::SeqState) -> C::Result {
        self.base.complete(&shared.0, state)
    }

    unsafe fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
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
