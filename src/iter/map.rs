use super::internal::*;
use super::len::*;
use super::*;

pub trait MapOp<In>: Sync {
    type Output: Send;
    fn map(&self, value: In) -> Self::Output;
}

pub struct MapFn<F>(pub F);

impl<F, In, Out> MapOp<In> for MapFn<F>
    where F: Fn(In) -> Out + Sync,
          Out: Send
{
    type Output = Out;
    fn map(&self, value: In) -> Out {
        (self.0)(value)
    }
}

pub struct MapCloned;

impl<'a, T> MapOp<&'a T> for MapCloned
    where T: Clone + Send
{
    type Output = T;
    fn map(&self, value: &'a T) -> T {
        value.clone()
    }
}

pub struct MapInspect<F>(pub F);

impl<F, In> MapOp<In> for MapInspect<F>
    where F: Fn(&In) + Sync,
          In: Send
{
    type Output = In;
    fn map(&self, value: In) -> In {
        (self.0)(&value);
        value
    }
}

/// ////////////////////////////////////////////////////////////////////////

pub struct Map<M: ParallelIterator, F> {
    base: M,
    map_op: F,
}

/// Create a new `Map` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<M, F>(base: M, map_op: F) -> Map<M, F>
    where M: ParallelIterator
{
    Map {
        base: base,
        map_op: map_op,
    }
}

impl<M, F> ParallelIterator for Map<M, F>
    where M: ParallelIterator,
          F: MapOp<M::Item>
{
    type Item = F::Output;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = MapConsumer::new(consumer, &self.map_op);
        self.base.drive_unindexed(consumer1)
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}

impl<M, F> BoundedParallelIterator for Map<M, F>
    where M: BoundedParallelIterator,
          F: MapOp<M::Item>
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

impl<M, F> ExactParallelIterator for Map<M, F>
    where M: ExactParallelIterator,
          F: MapOp<M::Item>
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M, F> IndexedParallelIterator for Map<M, F>
    where M: IndexedParallelIterator,
          F: MapOp<M::Item>
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback {
            callback: callback,
            map_op: self.map_op,
        });

        struct Callback<CB, F> {
            callback: CB,
            map_op: F,
        }

        impl<I, F, CB> ProducerCallback<I> for Callback<CB, F>
            where F: MapOp<I>,
                  CB: ProducerCallback<F::Output>
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = I>
            {
                let producer = MapProducer {
                    base: base,
                    map_op: &self.map_op,
                };
                self.callback.callback(producer)
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct MapProducer<'m, P, F: 'm> {
    base: P,
    map_op: &'m F,
}

impl<'m, P, F> Producer for MapProducer<'m, P, F>
    where P: Producer,
          F: MapOp<P::Item>
{
    type Item = F::Output;
    type IntoIter = MapIter<'m, P::IntoIter, F>;

    fn into_iter(self) -> Self::IntoIter {
        MapIter {
            base: self.base.into_iter(),
            map_op: self.map_op,
        }
    }

    fn weighted(&self) -> bool {
        self.base.weighted()
    }

    fn cost(&mut self, len: usize) -> f64 {
        self.base.cost(len) * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapProducer {
             base: left,
             map_op: self.map_op,
         },
         MapProducer {
             base: right,
             map_op: self.map_op,
         })
    }
}

pub struct MapIter<'m, I, F: 'm> {
    base: I,
    map_op: &'m F,
}

impl<'m, I, F> Iterator for MapIter<'m, I, F>
    where I: Iterator,
          F: MapOp<I::Item>
{
    type Item = F::Output;
    fn next(&mut self) -> Option<Self::Item> {
        self.base.next().map(|value| self.map_op.map(value))
    }
}

impl<'m, I, F> DoubleEndedIterator for MapIter<'m, I, F>
    where I: DoubleEndedIterator,
          F: MapOp<I::Item>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.base.next_back().map(|value| self.map_op.map(value))
    }
}

impl<'m, I, F> ExactSizeIterator for MapIter<'m, I, F>
    where I: ExactSizeIterator,
          F: MapOp<I::Item>
{
    fn len(&self) -> usize {
        self.base.len()
    }
}


/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct MapConsumer<'m, C, F: 'm> {
    base: C,
    map_op: &'m F,
}

impl<'m, C, F> MapConsumer<'m, C, F> {
    fn new(base: C, map_op: &'m F) -> Self {
        MapConsumer {
            base: base,
            map_op: map_op,
        }
    }
}

impl<'m, I, C, F> Consumer<I> for MapConsumer<'m, C, F>
    where C: Consumer<F::Output>,
          F: MapOp<I>
{
    type Folder = MapFolder<'m, C::Folder, F>;
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
        (MapConsumer::new(left, self.map_op), MapConsumer::new(right, self.map_op), reducer)
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

impl<'m, I, C, F> UnindexedConsumer<I> for MapConsumer<'m, C, F>
    where C: UnindexedConsumer<F::Output>,
          F: MapOp<I>
{
    fn split_off_left(&self) -> Self {
        MapConsumer::new(self.base.split_off_left(), &self.map_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct MapFolder<'m, C, F: 'm> {
    base: C,
    map_op: &'m F,
}

impl<'m, I, C, F> Folder<I> for MapFolder<'m, C, F>
    where C: Folder<F::Output>,
          F: MapOp<I>
{
    type Result = C::Result;

    fn consume(self, item: I) -> Self {
        let map_op = self.map_op;
        let mapped_item = map_op.map(item);
        let base = self.base.consume(mapped_item);
        MapFolder {
            base: base,
            map_op: map_op,
        }
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
