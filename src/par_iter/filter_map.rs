use super::{ParallelIterator, ParallelIteratorState, ParallelLen};
use std::marker::PhantomData;

pub struct FilterMap<M, FILTER_OP> {
    base: M,
    filter_op: FILTER_OP,
}

impl<M, FILTER_OP> FilterMap<M, FILTER_OP> {
    pub fn new(base: M, filter_op: FILTER_OP) -> FilterMap<M, FILTER_OP> {
        FilterMap { base: base, filter_op: filter_op }
    }
}

impl<M, FILTER_OP, R> ParallelIterator for FilterMap<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync,
          R: Send,
{
    type Item = R;
    type Shared = FilterMapShared<M, FILTER_OP>;
    type State = FilterMapState<M, FILTER_OP>;

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (FilterMapShared { base: base_shared, filter_op: self.filter_op },
         FilterMapState { base: base_state, filter_op: PhantomFilterOp::new() })
    }
}

pub struct FilterMapShared<M, FILTER_OP>
    where M: ParallelIterator,
{
    base: M::Shared,
    filter_op: FILTER_OP,
}

pub struct FilterMapState<M, FILTER_OP>
    where M: ParallelIterator,
{
    base: M::State,
    filter_op: PhantomFilterOp<FILTER_OP>,
}

struct PhantomFilterOp<FILTER_OP>(PhantomData<*const FILTER_OP>);

impl<FILTER_OP> PhantomFilterOp<FILTER_OP> {
    fn new() -> PhantomFilterOp<FILTER_OP> {
        PhantomFilterOp(PhantomData)
    }
}

unsafe impl<FILTER_OP: Sync> Send for PhantomFilterOp<FILTER_OP> { }

impl<M, FILTER_OP, R> ParallelIteratorState for FilterMapState<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync,
{
    type Item = R;
    type Shared = FilterMapShared<M, FILTER_OP>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        self.base.len(&shared.base)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (FilterMapState { base: left, filter_op: PhantomFilterOp::new() },
         FilterMapState { base: right, filter_op: PhantomFilterOp::new() })
    }

    fn for_each<F>(self, shared: &Self::Shared, mut op: F)
        where F: FnMut(R)
    {
        self.base.for_each(&shared.base, |item| {
            if let Some(item) = (shared.filter_op)(item) {
                op(item);
            }
        });
    }
}
