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
    type Shared = P::Shared;
    type SeqState = P::SeqState;

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (WeightProducer { base: left }, WeightProducer { base: right })
    }

    unsafe fn start(&mut self, shared: &P::Shared) -> P::SeqState {
        self.base.start(shared)
    }

    unsafe fn produce(&mut self,
                      shared: &P::Shared,
                      state: &mut P::SeqState)
                      -> P::Item
    {
        self.base.produce(shared, state)
    }

    unsafe fn complete(self,
                       shared: &P::Shared,
                       state: P::SeqState) {
        self.base.complete(shared, state)
    }
}
