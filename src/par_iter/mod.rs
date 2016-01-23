#![allow(dead_code)]

//! The `ParallelIterator` module makes it easy to write parallel
//! programs using an iterator-style interface. To get access to all
//! the methods you want, the easiest is to write `use
//! rayon::par_iter::*;` at the top of your module, which will import
//! the various traits and methods you need.
//!
//! The submodules of this module mostly just contain implementaton
//! details of little interest to an end-user.

use std::f64;
use std::ops::Fn;
use self::collect::collect_into;
use self::enumerate::Enumerate;
use self::map::Map;
use self::reduce::{reduce, ReduceOp, SumOp, MulOp, MinOp, MaxOp, ReduceWithOp,
                   SUM, MUL, MIN, MAX};
use self::state::ParallelIteratorState;
use self::weight::Weight;
use self::zip::ZipIter;

pub mod collect;
pub mod enumerate;
pub mod len;
pub mod for_each;
pub mod reduce;
pub mod slice;
pub mod slice_mut;
pub mod state;
pub mod map;
pub mod weight;
pub mod zip;

#[cfg(test)]
mod test;

pub trait IntoParallelIterator {
    type Iter: ParallelIterator<Item=Self::Item>;
    type Item: Send;

    fn into_par_iter(self) -> Self::Iter;
}

pub trait IntoParallelRefIterator<'r> {
    type Iter: ParallelIterator<Item=&'r Self::Item>;
    type Item: Sync + 'r;

    fn par_iter(&'r self) -> Self::Iter;
}

pub trait IntoParallelRefMutIterator<'r> {
    type Iter: ParallelIterator<Item=&'r mut Self::Item>;
    type Item: Send + 'r;

    fn par_iter_mut(&'r mut self) -> Self::Iter;
}

/// The `ParallelIterator` interface.
pub trait ParallelIterator {
    type Item: Send;
    type Shared: Sync;
    type State: ParallelIteratorState<Shared=Self::Shared, Item=Self::Item> + Send;

    fn state(self) -> (Self::Shared, Self::State);

    /// Indicates the relative "weight" of producing each item in this
    /// parallel iterator. A higher weight will cause finer-grained
    /// parallel subtasks. 1.0 indicates something very cheap and
    /// uniform, like copying a value out of an array, or computing `x
    /// + 1`. If your tasks are either very expensive, or very
    /// unpredictable, you are better off with higher values. See also
    /// `weight_max`, which is a convenient shorthand to force the
    /// finest grained parallel execution posible. Tuning this value
    /// should not affect correctness but can improve (or hurt)
    /// performance.
    fn weight(self, scale: f64) -> Weight<Self>
        where Self: Sized
    {
        Weight::new(self, scale)
    }

    /// Shorthand for `self.weight(f64::INFINITY)`. This forces the
    /// smallest granularity of parallel execution, which makes sense
    /// when your parallel tasks are (potentially) very expensive to
    /// execute.
    fn weight_max(self) -> Weight<Self>
        where Self: Sized
    {
        self.weight(f64::INFINITY)
    }

    /// Yields an index along with each item.
    fn enumerate(self) -> Enumerate<Self>
        where Self: Sized
    {
        Enumerate::new(self)
    }

    /// Executes `OP` on each item produced by the iterator, in parallel.
    fn for_each<OP>(self, op: OP)
        where OP: Fn(Self::Item) + Sync, Self: Sized
    {
        for_each::for_each(self, &op)
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

    /// Iterate over tuples `(A, B)`, where the items `A` are from
    /// this iterator and `B` are from the iterator given as argument.
    /// Like the `zip` method on ordinary iterators, if the two
    /// iterators are of unequal length, you only get the items they
    /// have in common.
    fn zip<ZIP_OP>(self, zip_op: ZIP_OP) -> ZipIter<Self, ZIP_OP::Iter>
        where Self: Sized, ZIP_OP: IntoParallelIterator
    {
        ZipIter::new(self, zip_op.into_par_iter())
    }
}



