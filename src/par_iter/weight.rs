use super::*;
use super::internal::*;
use super::util::*;

pub struct Weight<M> {
    base: M,
    weight: f64,
}

impl<M> Weight<M> {
    pub fn new(base: M, weight: f64) -> Weight<M> {
        Weight { base: base, weight: weight }
    }
}

impl<M> ParallelIterator for Weight<M>
    where M: ParallelIterator,
{
    type Item = M::Item;

    fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result {
        let consumer1: WeightConsumer<C> = WeightConsumer::new(consumer);
        let shared1 = (shared, self.weight);
        self.base.drive_unindexed(consumer1, &shared1)
    }
}

unsafe impl<M: BoundedParallelIterator> BoundedParallelIterator for Weight<M> {
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        let consumer1: WeightConsumer<C> = WeightConsumer::new(consumer);
        let shared1 = (shared, self.weight);
        self.base.drive(consumer1, &shared1)
    }
}

unsafe impl<M: ExactParallelIterator> ExactParallelIterator for Weight<M> {
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M: IndexedParallelIterator> IndexedParallelIterator for Weight<M> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback { weight: self.weight, callback: callback });

        struct Callback<CB> {
            weight: f64,
            callback: CB
        }

        impl<ITEM,CB> ProducerCallback<ITEM> for Callback<CB>
            where CB: ProducerCallback<ITEM>
        {
            type Output = CB::Output;

            fn with_producer<'p, P>(self, base: P, shared: &'p P::Shared) -> Self::Output
                where P: Producer<'p, Item=ITEM>
            {
                self.callback.with_producer(WeightProducer { base: base, phantoms: PhantomType::new() },
                                      &(shared, self.weight))
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct WeightProducer<'p, P> {
    base: P,
    phantoms: PhantomType<&'p ()>
}

impl<'w, 'p: 'w, P: Producer<'p>> Producer<'w> for WeightProducer<'p, P> {
    type Item = P::Item;
    type Shared = (&'p P::Shared, f64);

    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64 {
        self.base.cost(&shared.0, len) * shared.1
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (WeightProducer { base: left, phantoms: PhantomType::new() },
         WeightProducer { base: right, phantoms: PhantomType::new() })
    }

    unsafe fn produce(&mut self, shared: &Self::Shared) -> P::Item {
        self.base.produce(&shared.0)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct WeightConsumer<'w, 'c, C>
    where C: Consumer<'c>, 'c: 'w
{
    base: C,
    phantoms: PhantomType<&'w &'c ()>
}

impl<'w, 'c, C> WeightConsumer<'w, 'c, C>
    where C: Consumer<'c>, 'c: 'w
{
    fn new(base: C) -> WeightConsumer<'w, 'c, C> {
        WeightConsumer { base: base, phantoms: PhantomType::new() }
    }
}

impl<'w, 'c, C> Consumer<'w> for WeightConsumer<'w, 'c, C>
    where C: Consumer<'c>, 'c: 'w
{
    type Item = C::Item;
    type Shared = (&'c C::Shared, f64);
    type SeqState = C::SeqState;
    type Result = C::Result;

    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        self.base.cost(&shared.0, cost) * shared.1
    }

    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.0, index);
        (WeightConsumer::new(left), WeightConsumer::new(right))
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
        self.base.consume(&shared.0, state, item)
    }

    unsafe fn complete(self, shared: &Self::Shared, state: C::SeqState) -> C::Result {
        self.base.complete(&shared.0, state)
    }

    unsafe fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
        C::reduce(&shared.0, left, right)
    }
}

impl<'w, 'c, C> UnindexedConsumer<'w> for WeightConsumer<'w, 'c, C>
    where C: UnindexedConsumer<'c>, 'c: 'w
{
    fn split(&self) -> Self {
        WeightConsumer::new(self.base.split())
    }
}
