use super::internal::*;
use super::*;

/// Specifies a "map operator", transforming values into something else.
///
/// Implementing this trait is not permitted outside of `rayon`.
pub trait MapOp<In>: Sync {
    type Output: Send;
    fn map(&self, value: In) -> Self::Output;
    private_decl!{}
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
    private_impl!{}
}

pub struct MapCloned;

impl<'a, T> MapOp<&'a T> for MapCloned
    where T: Clone + Send
{
    type Output = T;
    fn map(&self, value: &'a T) -> T {
        value.clone()
    }
    private_impl!{}
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
    private_impl!{}
}

/// ////////////////////////////////////////////////////////////////////////

pub struct Map<I: ParallelIterator, F> {
    base: I,
    map_op: F,
}

/// Create a new `Map` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I, F>(base: I, map_op: F) -> Map<I, F>
    where I: ParallelIterator
{
    Map {
        base: base,
        map_op: map_op,
    }
}

impl<I, F> ParallelIterator for Map<I, F>
    where I: ParallelIterator,
          F: MapOp<I::Item>
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

impl<I, F> BoundedParallelIterator for Map<I, F>
    where I: BoundedParallelIterator,
          F: MapOp<I::Item>
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

impl<I, F> ExactParallelIterator for Map<I, F>
    where I: ExactParallelIterator,
          F: MapOp<I::Item>
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<I, F> IndexedParallelIterator for Map<I, F>
    where I: IndexedParallelIterator,
          F: MapOp<I::Item>
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

        impl<T, F, CB> ProducerCallback<T> for Callback<CB, F>
            where F: MapOp<T>,
                  CB: ProducerCallback<F::Output>
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = T>
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

struct MapProducer<'f, P, F: 'f> {
    base: P,
    map_op: &'f F,
}

impl<'f, P, F> Producer for MapProducer<'f, P, F>
    where P: Producer,
          F: MapOp<P::Item>
{
    type Item = F::Output;
    type IntoIter = MapIter<'f, P::IntoIter, F>;

    fn into_iter(self) -> Self::IntoIter {
        MapIter {
            base: self.base.into_iter(),
            map_op: self.map_op,
        }
    }

    fn min_len(&self) -> usize {
        self.base.min_len()
    }
    fn max_len(&self) -> usize {
        self.base.max_len()
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

pub struct MapIter<'f, T, F: 'f> {
    base: T,
    map_op: &'f F,
}

impl<'f, T, F> Iterator for MapIter<'f, T, F>
    where T: Iterator,
          F: MapOp<T::Item>
{
    type Item = F::Output;
    fn next(&mut self) -> Option<Self::Item> {
        self.base.next().map(|value| self.map_op.map(value))
    }
}

impl<'f, T, F> DoubleEndedIterator for MapIter<'f, T, F>
    where T: DoubleEndedIterator,
          F: MapOp<T::Item>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.base.next_back().map(|value| self.map_op.map(value))
    }
}

impl<'f, T, F> ExactSizeIterator for MapIter<'f, T, F>
    where T: ExactSizeIterator,
          F: MapOp<T::Item>
{
    fn len(&self) -> usize {
        self.base.len()
    }
}


/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct MapConsumer<'f, C, F: 'f> {
    base: C,
    map_op: &'f F,
}

impl<'f, C, F> MapConsumer<'f, C, F> {
    fn new(base: C, map_op: &'f F) -> Self {
        MapConsumer {
            base: base,
            map_op: map_op,
        }
    }
}

impl<'f, T, C, F> Consumer<T> for MapConsumer<'f, C, F>
    where C: Consumer<F::Output>,
          F: MapOp<T>
{
    type Folder = MapFolder<'f, C::Folder, F>;
    type Reducer = C::Reducer;
    type Result = C::Result;

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

impl<'f, T, C, F> UnindexedConsumer<T> for MapConsumer<'f, C, F>
    where C: UnindexedConsumer<F::Output>,
          F: MapOp<T>
{
    fn split_off_left(&self) -> Self {
        MapConsumer::new(self.base.split_off_left(), &self.map_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct MapFolder<'f, C, F: 'f> {
    base: C,
    map_op: &'f F,
}

impl<'f, T, C, F> Folder<T> for MapFolder<'f, C, F>
    where C: Folder<F::Output>,
          F: MapOp<T>
{
    type Result = C::Result;

    fn consume(self, item: T) -> Self {
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
