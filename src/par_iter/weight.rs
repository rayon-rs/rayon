use super::*;
use super::len::ParallelLen;
use super::state::*;

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
    type Shared = WeightShared<M>;
    type State = WeightState<M>;

    fn drive<C: Consumer<Item=Self::Item>>(self, consumer: C) -> C::Result {
        unimplemented!()
    }

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (WeightShared { base: base_shared, weight: self.weight },
         WeightState { base: base_state })
    }
}

unsafe impl<M: BoundedParallelIterator> BoundedParallelIterator for Weight<M> {
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }
}

unsafe impl<M: ExactParallelIterator> ExactParallelIterator for Weight<M> {
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M: PullParallelIterator> PullParallelIterator for Weight<M> {
    type Producer = WeightProducer<M::Producer>;

    fn into_producer(self) -> (Self::Producer, <Self::Producer as Producer>::Shared) {
        let (base, shared) = self.base.into_producer();
        (WeightProducer { base: base }, (self.weight, shared))
    }
}

pub struct WeightState<M>
    where M: ParallelIterator
{
    base: M::State,
}

pub struct WeightShared<M>
    where M: ParallelIterator
{
    base: M::Shared,
    weight: f64,
}

unsafe impl<M> ParallelIteratorState for WeightState<M>
    where M: ParallelIterator,
{
    type Item = M::Item;
    type Shared = WeightShared<M>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        let mut l = self.base.len(&shared.base);
        l.cost *= shared.weight;
        l
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (WeightState { base: left }, WeightState { base: right })
    }

    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item> {
        self.base.next(&shared.base)
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct WeightProducer<P: Producer> {
    base: P
}

impl<P: Producer> Producer for WeightProducer<P>
{
    type Item = P::Item;
    type Shared = (f64, P::Shared);

    unsafe fn cost(&mut self, shared: &Self::Shared, items: usize) -> f64 {
        self.base.cost(&shared.1, items) * shared.0
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (WeightProducer { base: left }, WeightProducer { base: right })
    }

    unsafe fn produce(&mut self, shared: &Self::Shared) -> P::Item {
        self.base.produce(&shared.1)
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct WeightConsumer<C>
    where C: Consumer,
{
    base: C,
}

impl<C> WeightConsumer<C>
    where C: Consumer,
{
    fn new(base: C) -> WeightConsumer<C> {
        WeightConsumer { base: base, }
    }
}

impl<C> Consumer for WeightConsumer<C>
    where C: Consumer,
{
    type Item = C::Item;
    type Shared = (f64, C::Shared);
    type SeqState = C::SeqState;
    type Result = C::Result;

    unsafe fn cost(&mut self, shared: &Self::Shared, items: usize) -> f64 {
        self.base.cost(&shared.1, items) * shared.0
    }

    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.1, index);
        (WeightConsumer::new(left), WeightConsumer::new(right))
    }

    unsafe fn start(&mut self, shared: &Self::Shared) -> C::SeqState {
        self.base.start(&shared.1)
    }

    unsafe fn consume(&mut self,
                      shared: &Self::Shared,
                      state: C::SeqState,
                      item: Self::Item)
                      -> C::SeqState
    {
        self.base.consume(&shared.1, state, item)
    }

    unsafe fn complete(self, shared: &Self::Shared, state: C::SeqState) -> C::Result {
        self.base.complete(&shared.1, state)
    }

    unsafe fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
        C::reduce(&shared.1, left, right)
    }
}
