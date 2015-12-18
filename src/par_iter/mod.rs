#![allow(dead_code)]

use std::ops::Fn;
use self::reduce::{SumOp, MulOp, MinOp, MaxOp, ReduceWithOp};

mod collect;
mod len;
mod reduce;
mod slice;
mod map;

#[cfg(test)]
mod test;

pub use self::collect::collect_into;
pub use self::len::ParallelLen;
pub use self::len::THRESHOLD;
pub use self::map::Map;
pub use self::reduce::reduce;
pub use self::reduce::ReduceOp;
pub use self::reduce::{SUM, MUL, MIN, MAX};

pub trait IntoParallelIterator {
    type Iter: ParallelIterator<Item=Self::Item>;
    type Item;

    fn into_par_iter(self) -> Self::Iter;
}

pub trait ParallelIterator {
    type Item;
    type Shared: Sync;
    type State: ParallelIteratorState<Shared=Self::Shared, Item=Self::Item> + Send;

    fn state(self) -> (Self::Shared, Self::State);

    fn map<MAP_OP,R>(self, map_op: MAP_OP) -> Map<Self, MAP_OP>
        where MAP_OP: Fn(Self::Item) -> R, Self: Sized
    {
        Map::new(self, map_op)
    }

    fn collect_into(self, target: &mut Vec<Self::Item>)
        where Self: Sized
    {
        collect_into(self, target);
    }

    fn reduce_with<OP>(self, op: OP) -> Option<Self::Item>
        where Self: Sized, OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync,
    {
        reduce(self.map(Some), &ReduceWithOp::new(op))
    }

    fn sum(self) -> Self::Item
        where Self: Sized, SumOp: ReduceOp<Self::Item>
    {
        reduce(self, SUM)
    }

    fn mul(self) -> Self::Item
        where Self: Sized, MulOp: ReduceOp<Self::Item>
    {
        reduce(self, MUL)
    }

    fn min(self) -> Self::Item
        where Self: Sized, MinOp: ReduceOp<Self::Item>
    {
        reduce(self, MIN)
    }

    fn max(self) -> Self::Item
        where Self: Sized, MaxOp: ReduceOp<Self::Item>
    {
        reduce(self, MAX)
    }

    fn reduce<REDUCE_OP>(self, reduce_op: &REDUCE_OP) -> Self::Item
        where Self: Sized, REDUCE_OP: ReduceOp<Self::Item>
    {
        reduce(self, reduce_op)
    }
}

pub trait ParallelIteratorState: Sized {
    type Item;
    type Shared: Sync;

    fn len(&mut self) -> ParallelLen;

    fn split_at(self, index: usize) -> (Self, Self);

    fn for_each<OP>(self, shared: &Self::Shared, op: OP)
        where OP: FnMut(Self::Item);
}



