use super::*;
use super::len::ParallelLen;
use super::state::ParallelIteratorState;
use std::marker::PhantomData;

pub struct Filter<M, FILTER_OP> {
    base: M,
    filter_op: FILTER_OP,
}

impl<M, FILTER_OP> Filter<M, FILTER_OP> {
    pub fn new(base: M, filter_op: FILTER_OP) -> Filter<M, FILTER_OP> {
        Filter { base: base, filter_op: filter_op }
    }
}

impl<M, FILTER_OP> ParallelIterator for Filter<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync,
{
    type Item = M::Item;
    type Shared = FilterShared<M, FILTER_OP>;
    type State = FilterState<M, FILTER_OP>;

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (FilterShared { base: base_shared, filter_op: self.filter_op },
         FilterState { base: base_state, filter_op: PhantomFilterOp::new() })
    }
}

unsafe impl<M, FILTER_OP> BoundedParallelIterator for Filter<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync
{}

pub struct FilterShared<M, FILTER_OP>
    where M: ParallelIterator,
{
    base: M::Shared,
    filter_op: FILTER_OP,
}

pub struct FilterState<M, FILTER_OP>
    where M: ParallelIterator,
{
    base: M::State,
    filter_op: PhantomFilterOp<FILTER_OP>,
}

// Wrapper for `PhantomData` to allow `FilterState` to impl `Send`
struct PhantomFilterOp<FILTER_OP>(PhantomData<*const FILTER_OP>);

impl<FILTER_OP> PhantomFilterOp<FILTER_OP> {
    fn new() -> PhantomFilterOp<FILTER_OP> {
        PhantomFilterOp(PhantomData)
    }
}

unsafe impl<FILTER_OP: Sync> Send for PhantomFilterOp<FILTER_OP> { }

unsafe impl<M, FILTER_OP> ParallelIteratorState for FilterState<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(&M::Item) -> bool + Sync
{
    type Item = M::Item;
    type Shared = FilterShared<M, FILTER_OP>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        self.base.len(&shared.base)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (FilterState { base: left, filter_op: PhantomFilterOp::new() },
         FilterState { base: right, filter_op: PhantomFilterOp::new() })
    }

    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item> {
        while let Some(base) = self.base.next(&shared.base) {
            if (shared.filter_op)(&base) {
                return Some(base);
            }
        }
        None
    }
}
