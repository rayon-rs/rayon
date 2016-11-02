use super::*;
use super::len::*;
use super::internal::*;

pub trait MapOp<In>: Sync {
    type Output: Send;
    fn map(&self, value: In) -> Self::Output;
}

pub struct MapFn<F>(pub F);

impl<F, In, Out> MapOp<In> for MapFn<F>
    where F: Fn(In) -> Out + Sync,
          Out: Send,
{
    type Output = Out;
    fn map(&self, value: In) -> Out {
        (self.0)(value)
    }
}

pub struct MapCloned;

impl<'a, T> MapOp<&'a T> for MapCloned
    where T: Clone + Send,
{
    type Output = T;
    fn map(&self, value: &'a T) -> T {
        value.clone()
    }
}

pub struct MapInspect<F>(pub F);

impl<F, In> MapOp<In> for MapInspect<F>
    where F: Fn(&In) + Sync,
          In: Send,
{
    type Output = In;
    fn map(&self, value: In) -> In {
        (self.0)(&value);
        value
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct Map<M, MAP_OP> {
    base: M,
    map_op: MAP_OP,
}

impl<M, MAP_OP> Map<M, MAP_OP> {
    pub fn new(base: M, map_op: MAP_OP) -> Map<M, MAP_OP> {
        Map { base: base, map_op: map_op }
    }
}

impl<M, MAP_OP> ParallelIterator for Map<M, MAP_OP>
    where M: ParallelIterator,
          MAP_OP: MapOp<M::Item>,
{
    type Item = MAP_OP::Output;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = MapConsumer::new(consumer, &self.map_op);
        self.base.drive_unindexed(consumer1)
    }
}

impl<M, MAP_OP> BoundedParallelIterator for Map<M, MAP_OP>
    where M: BoundedParallelIterator,
          MAP_OP: MapOp<M::Item>,
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

impl<M, MAP_OP> ExactParallelIterator for Map<M, MAP_OP>
    where M: ExactParallelIterator,
          MAP_OP: MapOp<M::Item>,
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M, MAP_OP> IndexedParallelIterator for Map<M, MAP_OP>
    where M: IndexedParallelIterator,
          MAP_OP: MapOp<M::Item>,
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback { callback: callback, map_op: self.map_op });

        struct Callback<CB, MAP_OP> {
            callback: CB,
            map_op: MAP_OP,
        }

        impl<ITEM, MAP_OP, CB> ProducerCallback<ITEM> for Callback<CB, MAP_OP>
            where MAP_OP: MapOp<ITEM>,
                  CB: ProducerCallback<MAP_OP::Output>
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

impl<'m, P, MAP_OP> Producer for MapProducer<'m, P, MAP_OP>
    where P: Producer,
          MAP_OP: MapOp<P::Item>,
{
    fn weighted(&self) -> bool {
        self.base.weighted()
    }

    fn cost(&mut self, len: usize) -> f64 {
        self.base.cost(len) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapProducer { base: left, map_op: self.map_op, },
         MapProducer { base: right, map_op: self.map_op, })
    }
}

impl<'m, P, MAP_OP> IntoIterator for MapProducer<'m, P, MAP_OP>
    where P: Producer,
          MAP_OP: MapOp<P::Item>,
{
    type Item = MAP_OP::Output;
    type IntoIter = MapIter<'m, P::IntoIter, MAP_OP>;

    fn into_iter(self) -> Self::IntoIter {
        MapIter {
            base: self.base.into_iter(),
            map_op: self.map_op,
        }
    }
}


pub struct MapIter<'m, I, MAP_OP: 'm> {
    base: I,
    map_op: &'m MAP_OP,
}

impl<'m, I, MAP_OP> Iterator for MapIter<'m, I, MAP_OP>
    where I: Iterator,
          MAP_OP: MapOp<I::Item>,
{
    type Item = MAP_OP::Output;
    fn next(&mut self) -> Option<Self::Item> {
        self.base.next().map(|value| self.map_op.map(value))
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

impl<'m, ITEM, C, MAP_OP> Consumer<ITEM> for MapConsumer<'m, C, MAP_OP>
    where C: Consumer<MAP_OP::Output>,
          MAP_OP: MapOp<ITEM>,
{
    type Folder = MapFolder<'m, C::Folder, MAP_OP>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn weighted(&self) -> bool {
        self.base.weighted()
    }

    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (MapConsumer::new(left, self.map_op),
         MapConsumer::new(right, self.map_op),
         reducer)
    }

    fn into_folder(self) -> Self::Folder {
        MapFolder {
            base: self.base.into_folder(),
            map_op: self.map_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'m, ITEM, C, MAP_OP> UnindexedConsumer<ITEM>
    for MapConsumer<'m, C, MAP_OP>
    where C: UnindexedConsumer<MAP_OP::Output>,
          MAP_OP: MapOp<ITEM>,
{
    fn split_off(&self) -> Self {
        MapConsumer::new(self.base.split_off(), &self.map_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct MapFolder<'m, C, MAP_OP: 'm>
{
    base: C,
    map_op: &'m MAP_OP,
}

impl<'m, ITEM, C, MAP_OP> Folder<ITEM> for MapFolder<'m, C, MAP_OP>
    where C: Folder<MAP_OP::Output>,
          MAP_OP: MapOp<ITEM>,
{
    type Result = C::Result;

    fn consume(self, item: ITEM) -> Self {
        let map_op = self.map_op;
        let mapped_item = map_op.map(item);
        let base = self.base.consume(mapped_item);
        MapFolder { base: base, map_op: map_op }
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

