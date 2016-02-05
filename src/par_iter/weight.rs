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

    fn drive_unindexed<'c, C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<'c, Item=Self::Item>
    {
        let consumer1 = WeightConsumer::new(consumer, self.weight);
        self.base.drive_unindexed(consumer1)
    }
}

unsafe impl<M: BoundedParallelIterator> BoundedParallelIterator for Weight<M> {
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<'c, C>(self, consumer: C) -> C::Result
        where C: Consumer<'c, Item=Self::Item>
    {
        let consumer1: WeightConsumer<C> = WeightConsumer::new(consumer, self.weight);
        self.base.drive(consumer1)
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

            fn callback<'p, P>(self, base: P, shared: &'p P::Shared) -> Self::Output
                where P: Producer<'p, Item=ITEM>
            {
                self.callback.callback(WeightProducer { base: base,
                                                        weight: self.weight,
                                                        phantoms: PhantomType::new() },
                                       shared)
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct WeightProducer<'p, P> {
    base: P,
    weight: f64,
    phantoms: PhantomType<&'p ()>
}

impl<'w, 'p: 'w, P: Producer<'p>> Producer<'w> for WeightProducer<'p, P> {
    type Item = P::Item;
    type Shared = P::Shared;

    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64 {
        self.base.cost(shared, len) * self.weight
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (WeightProducer { base: left,
                          weight: self.weight,
                          phantoms: PhantomType::new() },
         WeightProducer { base: right,
                          weight: self.weight,
                          phantoms: PhantomType::new() })
    }

    unsafe fn produce(&mut self, shared: &Self::Shared) -> P::Item {
        self.base.produce(shared)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct WeightConsumer<'w, 'c, C>
    where C: Consumer<'c>, 'c: 'w
{
    base: C,
    weight: f64,
    phantoms: PhantomType<&'w &'c ()>
}

impl<'w, 'c, C> WeightConsumer<'w, 'c, C>
    where C: Consumer<'c>, 'c: 'w
{
    fn new(base: C, weight: f64) -> WeightConsumer<'w, 'c, C> {
        WeightConsumer { base: base, weight: weight, phantoms: PhantomType::new() }
    }
}

impl<'w, 'c, C> Consumer<'w> for WeightConsumer<'w, 'c, C>
    where C: Consumer<'c>, 'c: 'w
{
    type Item = C::Item;
    type SeqState = C::SeqState;
    type Result = C::Result;

    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * self.weight
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (WeightConsumer::new(left, self.weight),
         WeightConsumer::new(right, self.weight))
    }

    unsafe fn start(&mut self) -> C::SeqState {
        self.base.start()
    }

    unsafe fn consume(&mut self,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        self.base.consume(state, item)
    }

    unsafe fn complete(self, state: C::SeqState) -> C::Result {
        self.base.complete(state)
    }

    unsafe fn reduce(left: C::Result, right: C::Result) -> C::Result {
        C::reduce(left, right)
    }
}

impl<'w, 'c, C> UnindexedConsumer<'w> for WeightConsumer<'w, 'c, C>
    where C: UnindexedConsumer<'c>, 'c: 'w
{
    fn split(&self) -> Self {
        WeightConsumer::new(self.base.split(), self.weight)
    }
}
