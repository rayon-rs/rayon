use super::{ParallelIterator, ParallelIteratorState, ParallelLen};
use std::marker::PhantomData;

pub struct Filter<M, FILTER_PRED> {
    base: M,
    pred: FILTER_PRED,
}

impl<M, FILTER_PRED> Filter<M, FILTER_PRED> {
    pub fn new(base: M, pred: FILTER_PRED) -> Filter<M, FILTER_PRED> {
        Filter { base: base, pred: pred }
    }
}

impl<M, FILTER_PRED> ParallelIterator for Filter<M, FILTER_PRED>
    where M: ParallelIterator,
          FILTER_PRED: Fn(&M::Item) -> bool + Sync,
{
    type Item = M::Item;
    type Shared = FilterShared<M, FILTER_PRED>;
    type State = FilterState<M, FILTER_PRED>;

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (FilterShared { base: base_shared, pred: self.pred },
         FilterState { base: base_state, pred: PhantomFilterPred::new() })
    }
}

pub struct FilterShared<M, FILTER_PRED>
    where M: ParallelIterator,
{
    base: M::Shared,
    pred: FILTER_PRED
}

pub struct FilterState<M, FILTER_PRED>
    where M: ParallelIterator,
{
    base: M::State,
    pred: PhantomFilterPred<FILTER_PRED>,
}

struct PhantomFilterPred<FILTER_PRED>(PhantomData<*const FILTER_PRED>);

impl<FILTER_PRED> PhantomFilterPred<FILTER_PRED> {
    fn new() -> PhantomFilterPred<FILTER_PRED> {
        PhantomFilterPred(PhantomData)
    }
}

unsafe impl<FILTER_PRED: Sync> Send for PhantomFilterPred<FILTER_PRED> { }

impl<M, FILTER_PRED> ParallelIteratorState for FilterState<M, FILTER_PRED>
    where M: ParallelIterator,
          FILTER_PRED: Fn(&M::Item) -> bool + Sync,
{
    type Item = M::Item;
    type Shared = FilterShared<M, FILTER_PRED>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        self.base.len(&shared.base)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (FilterState { base: left, pred: PhantomFilterPred::new() },
         FilterState { base: right, pred: PhantomFilterPred::new() })
    }

    fn for_each<F>(self, shared: &Self::Shared, mut op: F)
        where F: FnMut(Self::Item),
    {
        self.base.for_each(&shared.base, |item| {
            if (shared.pred)(&item) {
                op(item);
            }
        });
    }
}
