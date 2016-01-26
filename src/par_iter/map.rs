use super::*;
use super::len::*;
use super::state::*;
use super::util::PhantomType;
use std::marker::PhantomData;

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
    type Shared = MapShared<M, MAP_OP>;
    type State = MapState<M, MAP_OP>;

    fn drive<C: Consumer<Item=Self::Item>>(self, consumer: C) -> C::Result {
        unimplemented!()
    }

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (MapShared { base: base_shared, map_op: self.map_op },
         MapState { base: base_state, map_op: PhantomMapOp::new() })
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

impl<M, MAP_OP, R, P> PullParallelIterator for Map<M, MAP_OP>
    where M: PullParallelIterator<Producer=P>,
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

pub struct MapShared<M, MAP_OP>
    where M: ParallelIterator,
{
    base: M::Shared,
    map_op: MAP_OP,
}

pub struct MapState<M, MAP_OP>
    where M: ParallelIterator,
{
    base: M::State,
    map_op: PhantomMapOp<MAP_OP>,
}

// Wrapper for `PhantomData` to allow `MapState` to impl `Send`
struct PhantomMapOp<MAP_OP>(PhantomData<*const MAP_OP>);

impl<MAP_OP> PhantomMapOp<MAP_OP> {
    fn new() -> PhantomMapOp<MAP_OP> {
        PhantomMapOp(PhantomData)
    }
}

unsafe impl<MAP_OP: Sync> Send for PhantomMapOp<MAP_OP> { }

unsafe impl<M, MAP_OP, R> ParallelIteratorState for MapState<M, MAP_OP>
    where M: ParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync,
{
    type Item = R;
    type Shared = MapShared<M, MAP_OP>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        self.base.len(&shared.base)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapState { base: left, map_op: PhantomMapOp::new() },
         MapState { base: right, map_op: PhantomMapOp::new() })
    }

    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item> {
        self.base.next(&shared.base)
                 .map(|base| (shared.map_op)(base))
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

    unsafe fn cost(&mut self, shared: &Self::Shared, items: usize) -> f64 {
        self.base.cost(&shared.0, items) * FUNC_ADJUSTMENT
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

struct MapConsumer<ITEM, C, MAP_OP>
    where C: Consumer, MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    base: C,
    phantoms: PhantomType<(ITEM, MAP_OP)>,
}

impl<ITEM, C, MAP_OP> MapConsumer<ITEM, C, MAP_OP>
    where C: Consumer, MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    fn new(base: C) -> MapConsumer<ITEM, C, MAP_OP> {
        MapConsumer { base: base, phantoms: PhantomType::new() }
    }
}

struct MapConsumerShared<ITEM, C, MAP_OP>
    where C: Consumer, MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    base: C::Shared,
    filter_op: MAP_OP,
    phantoms: PhantomType<ITEM>
}

impl<ITEM, C, MAP_OP> Consumer for MapConsumer<ITEM, C, MAP_OP>
    where C: Consumer, MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    type Item = ITEM;
    type Shared = MapConsumerShared<ITEM, C, MAP_OP>;
    type SeqState = C::SeqState;
    type Result = C::Result;

    unsafe fn cost(&mut self, shared: &Self::Shared, items: usize) -> f64 {
        self.base.cost(&shared.base, items) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.base, index);
        (MapConsumer::new(left), MapConsumer::new(right))
    }

    unsafe fn start(&mut self, shared: &Self::Shared) -> C::SeqState {
        self.base.start(&shared.base)
    }

    unsafe fn consume(&mut self,
                      shared: &Self::Shared,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        let mapped_item = (shared.filter_op)(item);
        self.base.consume(&shared.base, state, mapped_item)
    }

    unsafe fn complete(self, shared: &Self::Shared, state: C::SeqState) -> C::Result {
        self.base.complete(&shared.base, state)
    }

    unsafe fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
        C::reduce(&shared.base, left, right)
    }
}

