use super::*;
use super::len::*;
use super::state::*;
use super::util::PhantomType;
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
    type Shared = FilterShared<M, FILTER_OP>;
    type State = FilterState<M, FILTER_OP>;

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        let consumer: FilterMapConsumer<M::Item, C, FILTER_OP> = FilterMapConsumer::new(consumer);
        let shared = (shared, &self.filter_op);
        self.base.drive(consumer, &shared)
    }

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (FilterShared { base: base_shared, filter_op: self.filter_op },
         FilterState { base: base_state, filter_op: PhantomFilterOp::new() })
    }
}

unsafe impl<M, FILTER_OP, R> BoundedParallelIterator for FilterMap<M, FILTER_OP>
    where M: BoundedParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync,
          R: Send,
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }
}

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

unsafe impl<M, FILTER_OP, R> ParallelIteratorState for FilterState<M, FILTER_OP>
    where M: ParallelIterator,
          FILTER_OP: Fn(M::Item) -> Option<R> + Sync
{
    type Item = R;
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
            if let Some(mapped) = (shared.filter_op)(base) {
                return Some(mapped);
            }
        }
        None
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync, 'c: 'f
{
    base: C,
    phantoms: PhantomType<(&'f &'c (), ITEM, FILTER_OP)>,
}

impl<'f, 'c, ITEM, C, FILTER_OP> FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync, 'c: 'f
{
    fn new(base: C) -> FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP> {
        FilterMapConsumer { base: base, phantoms: PhantomType::new() }
    }
}

impl<'f, 'c, ITEM, C, FILTER_OP> Consumer<'f> for FilterMapConsumer<'f, 'c, ITEM, C, FILTER_OP>
    where C: Consumer<'c>, FILTER_OP: Fn(ITEM) -> Option<C::Item> + Sync, ITEM: 'f, FILTER_OP: 'f,
{
    type Item = ITEM;
    type Shared = (&'c C::Shared, &'f FILTER_OP);
    type SeqState = C::SeqState;
    type Result = C::Result;

    /// Cost to process `items` number of items.
    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        self.base.cost(&shared.0, cost) * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, shared: &Self::Shared, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(&shared.0, index);
        (FilterMapConsumer::new(left), FilterMapConsumer::new(right))
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
        if let Some(mapped_item) = (shared.1)(item) {
            self.base.consume(&shared.0, state, mapped_item)
        } else {
            state
        }
    }

    unsafe fn complete(self, shared: &Self::Shared, state: C::SeqState) -> C::Result {
        self.base.complete(&shared.0, state)
    }

    unsafe fn reduce(shared: &Self::Shared, left: C::Result, right: C::Result) -> C::Result {
        C::reduce(&shared.0, left, right)
    }
}
