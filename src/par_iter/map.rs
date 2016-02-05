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

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Item=Self::Item>
    {
        let consumer1 = MapConsumer::new(consumer, &self.map_op);
        self.base.drive_unindexed(consumer1)
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

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Item=Self::Item>
    {
        let consumer1 = MapConsumer::new(consumer, &self.map_op);
        self.base.drive(consumer1)
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
                let producer = MapProducer { base: base,
                                             map_op: &self.map_op,
                                             phantoms: PhantomType::new() };
                self.callback.callback(producer, &(shared, &self.map_op))
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct MapProducer<'m, 'p, P, MAP_OP, R>
    where P: Producer<'p>,
          MAP_OP: Fn(P::Item) -> R + Sync + 'm,
          R: Send,
          'p: 'm,
{
    base: P,
    map_op: &'m MAP_OP,
    phantoms: PhantomType<(&'p (), R)>,
}

impl<'m, 'p, P, MAP_OP, R> Producer<'m> for MapProducer<'m, 'p, P, MAP_OP, R>
    where P: Producer<'p>,
          MAP_OP: Fn(P::Item) -> R + Sync,
          R: Send,
          'p: 'm
{
    type Item = R;
    type Shared = (&'p P::Shared, &'m MAP_OP);

    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64 {
        self.base.cost(&shared.0, len) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapProducer { base: left, map_op: self.map_op, phantoms: PhantomType::new() },
         MapProducer { base: right, map_op: self.map_op, phantoms: PhantomType::new() })
    }

    unsafe fn produce(&mut self, shared: &Self::Shared) -> R {
        let item = self.base.produce(&shared.0);
        (shared.1)(item)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct MapConsumer<'m, ITEM, C, MAP_OP>
    where C: Consumer, MAP_OP: Fn(ITEM) -> C::Item + Sync + 'm,
{
    base: C,
    map_op: &'m MAP_OP,
    phantoms: PhantomType<ITEM>,
}

impl<'m, ITEM, C, MAP_OP> MapConsumer<'m, ITEM, C, MAP_OP>
    where C: Consumer, MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    fn new(base: C, map_op: &'m MAP_OP) -> MapConsumer<'m, ITEM, C, MAP_OP> {
        MapConsumer { base: base, map_op: map_op, phantoms: PhantomType::new() }
    }
}

impl<'m, ITEM, C, MAP_OP> Consumer for MapConsumer<'m, ITEM, C, MAP_OP>
    where C: Consumer,
          MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    type Item = ITEM;
    type SeqState = C::SeqState;
    type Result = C::Result;

    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapConsumer::new(left, self.map_op), MapConsumer::new(right, self.map_op))
    }

    unsafe fn start(&mut self) -> C::SeqState {
        self.base.start()
    }

    unsafe fn consume(&mut self,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        let mapped_item = (self.map_op)(item);
        self.base.consume(state, mapped_item)
    }

    unsafe fn complete(self, state: C::SeqState) -> C::Result {
        self.base.complete(state)
    }

    unsafe fn reduce(left: C::Result, right: C::Result) -> C::Result {
        C::reduce(left, right)
    }
}

impl<'m, ITEM, C, MAP_OP> UnindexedConsumer
    for MapConsumer<'m, ITEM, C, MAP_OP>
    where C: UnindexedConsumer,
          MAP_OP: Fn(ITEM) -> C::Item + Sync,
{
    fn split(&self) -> Self {
        MapConsumer::new(self.base.split(), &self.map_op)
    }
}
