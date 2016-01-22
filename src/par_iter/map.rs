use super::{ParallelIterator, ParallelIteratorState, ParallelLen};
use std::marker::PhantomData;

pub struct Map<M, MAP_OP> {
    base: M,
    map_op: MAP_OP,
}

impl<M, MAP_OP> Map<M, MAP_OP> {
    pub fn new(base: M, map_op: MAP_OP) -> Map<M, MAP_OP> {
        Map { base: base, map_op: map_op }
    }
}

impl<M, MAP_OP, R> ParallelIterator for Map<M, MAP_OP>
    where M: ParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync,
          R: Send,
{
    type Item = R;
    type Shared = MapShared<M, MAP_OP>;
    type State = MapState<M, MAP_OP>;

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (MapShared { base: base_shared, map_op: self.map_op },
         MapState { base: base_state, map_op: PhantomMap::new() })
    }
}

pub struct MapShared<M, MAP_OP>
    where M: ParallelIterator
{
    base: M::Shared,
    map_op: MAP_OP
}

pub struct MapState<M, MAP_OP>
    where M: ParallelIterator
{
    base: M::State,
    map_op: PhantomMap<MAP_OP>
}

pub struct PhantomMap<MAP_OP> {
    data: PhantomData<*const MAP_OP>
}

impl<MAP_OP> PhantomMap<MAP_OP> {
    fn new() -> PhantomMap<MAP_OP> {
        PhantomMap { data: PhantomData }
    }
}

unsafe impl<MAP_OP: Sync> Send for PhantomMap<MAP_OP> { }

unsafe impl<M, MAP_OP, R> ParallelIteratorState for MapState<M, MAP_OP>
    where M: ParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync
{
    type Item = R;
    type Shared = MapShared<M, MAP_OP>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        self.base.len(&shared.base)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapState { base: left, map_op: PhantomMap::new() },
         MapState { base: right, map_op: PhantomMap::new() })
    }

    fn for_each<F>(self, shared: &Self::Shared, mut op: F)
        where F: FnMut(R)
    {
        self.base.for_each(&shared.base, |item| {
            op((shared.map_op)(item));
        });
    }
}
