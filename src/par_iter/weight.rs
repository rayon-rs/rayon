use super::{ParallelIterator, ParallelIteratorState, ParallelLen};

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

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (WeightShared { base: base_shared, weight: self.weight },
         WeightState { base: base_state })
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

impl<M> ParallelIteratorState for WeightState<M>
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

    fn for_each<F>(self, shared: &Self::Shared, op: F)
        where F: FnMut(Self::Item)
    {
        self.base.for_each(&shared.base, op)
    }
}
