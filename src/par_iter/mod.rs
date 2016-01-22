#![allow(dead_code)]

use std::ops::Fn;
use self::reduce::{SumOp, MulOp, MinOp, MaxOp, ReduceWithOp};

mod collect;
mod enumerate;
mod len;
mod reduce;
mod slice;
mod map;
mod weight;

#[cfg(test)]
mod test;

pub use self::collect::collect_into;
pub use self::enumerate::Enumerate;
pub use self::len::ParallelLen;
pub use self::len::THRESHOLD;
pub use self::map::Map;
pub use self::reduce::reduce;
pub use self::reduce::ReduceOp;
pub use self::reduce::{SUM, MUL, MIN, MAX};
pub use self::weight::Weight;

pub trait IntoParallelIterator {
    type Iter: ParallelIterator<Item=Self::Item>;
    type Item: Send;

    fn into_par_iter(self) -> Self::Iter;
}

/// The `ParallelIterator` interface.
pub trait ParallelIterator {
    type Item: Send;
    type Shared: Sync;
    type State: ParallelIteratorState<Shared=Self::Shared, Item=Self::Item> + Send;

    fn state(self) -> (Self::Shared, Self::State);

    /// Indicates the relative "weight" of producing each item in this
    /// parallel iterator. 1.0 indicates something very cheap, like
    /// copying a value out of an array, or computing `x + 1`. Tuning
    /// this value can affect how many subtasks are created and can
    /// improve performance.
    fn weight(self, scale: f64) -> Weight<Self>
        where Self: Sized
    {
        Weight::new(self, scale)
    }

    /// Yields an index along with each item.
    fn enumerate(self) -> Enumerate<Self>
        where Self: Sized
    {
        Enumerate::new(self)
    }

    /// Applies `map_op` to each item of his iterator, producing a new
    /// iterator with the results.
    fn map<MAP_OP,R>(self, map_op: MAP_OP) -> Map<Self, MAP_OP>
        where MAP_OP: Fn(Self::Item) -> R, Self: Sized
    {
        Map::new(self, map_op)
    }

    /// Collects the results of the iterator into the specified
    /// vector. The vector is always truncated before execution
    /// begins. If possible, reusing the vector across calls can lead
    /// to better performance since it reuses the same backing buffer.
    fn collect_into(self, target: &mut Vec<Self::Item>)
        where Self: Sized
    {
        collect_into(self, target);
    }

    /// Reduces the items in the iterator into one item using `op`.
    /// See also `sum`, `mul`, `min`, etc, which are slightly more
    /// efficient. Returns `None` if the iterator is empty.
    ///
    /// Note: unlike in a sequential iterator, the order in which `op`
    /// will be applied to reduce the result is not specified. So `op`
    /// should be commutative and associative or else the results will
    /// be non-deterministic.
    fn reduce_with<OP>(self, op: OP) -> Option<Self::Item>
        where Self: Sized, OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync,
    {
        reduce(self.map(Some), &ReduceWithOp::new(op))
    }

    /// Sums up the items in the iterator.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `+` operator is not truly commutative and
    /// associative (as is the case for floating point numbers), then
    /// the results are not fully deterministic.
    fn sum(self) -> Self::Item
        where Self: Sized, SumOp: ReduceOp<Self::Item>
    {
        reduce(self, SUM)
    }

    /// Multiplies all the items in the iterator.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `*` operator is not truly commutative and
    /// associative (as is the case for floating point numbers), then
    /// the results are not fully deterministic.
    fn mul(self) -> Self::Item
        where Self: Sized, MulOp: ReduceOp<Self::Item>
    {
        reduce(self, MUL)
    }

    /// Computes the minimum of all the items in the iterator.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `Ord` impl is not truly commutative and associative
    /// (as is the case for floating point numbers), then the results
    /// are not deterministic.
    fn min(self) -> Self::Item
        where Self: Sized, MinOp: ReduceOp<Self::Item>
    {
        reduce(self, MIN)
    }

    /// Computes the maximum of all the items in the iterator.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `Ord` impl is not truly commutative and associative
    /// (as is the case for floating point numbers), then the results
    /// are not deterministic.
    fn max(self) -> Self::Item
        where Self: Sized, MaxOp: ReduceOp<Self::Item>
    {
        reduce(self, MAX)
    }

    /// Reduces the items using the given "reduce operator". You may
    /// prefer `reduce_with` for a simpler interface.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `reduce_op` impl is not truly commutative and
    /// associative, then the results are not deterministic.
    fn reduce<REDUCE_OP>(self, reduce_op: &REDUCE_OP) -> Self::Item
        where Self: Sized, REDUCE_OP: ReduceOp<Self::Item>
    {
        reduce(self, reduce_op)
    }
}

/// The trait for types representing the internal *state* during
/// parallelization. This basically represents a group of tasks
/// to be done.
///
/// Note that this trait is declared as an **unsafe trait**. That
/// means that the trait is unsafe to implement. The reason is that
/// other bits of code, such as the `collect` routine on
/// `ParallelIterator`, rely on the `len` and `for_each` functions
/// being accurate and correct. For example, if the `len` function
/// reports that it will produce N items, then `for_each` *must*
/// produce `N` items or else the resulting vector will contain
/// uninitialized memory.
///
/// This trait is not really intended to be implemented outside of the
/// Rayon crate at this time. The precise safety requirements are kind
/// of ill-documented for this reason (i.e., they are ill-understood).
pub unsafe trait ParallelIteratorState: Sized {
    type Item;
    type Shared: Sync;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen;

    fn split_at(self, index: usize) -> (Self, Self);

    fn for_each<OP>(self, shared: &Self::Shared, op: OP)
        where OP: FnMut(Self::Item);
}



