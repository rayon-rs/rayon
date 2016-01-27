#![allow(dead_code)]

//! The `ParallelIterator` module makes it easy to write parallel
//! programs using an iterator-style interface. To get access to all
//! the methods you want, the easiest is to write `use
//! rayon::par_iter::*;` at the top of your module, which will import
//! the various traits and methods you need.
//!
//! The submodules of this module mostly just contain implementaton
//! details of little interest to an end-user. If you'd like to read
//! the code itself, the `internal` module and `README.md` file are a
//! good place to start.

use std::f64;
use std::ops::Fn;
use self::collect::collect_into;
use self::enumerate::Enumerate;
use self::filter::Filter;
use self::filter_map::FilterMap;
use self::flat_map::FlatMap;
use self::map::Map;
use self::reduce::{reduce, ReduceOp, SumOp, MulOp, MinOp, MaxOp, ReduceWithOp,
                   SUM, MUL, MIN, MAX};
use self::internal::*;
use self::weight::Weight;
use self::zip::ZipIter;

pub mod collect;
pub mod enumerate;
pub mod filter;
pub mod filter_map;
pub mod flat_map;
pub mod internal;
pub mod len;
pub mod for_each;
pub mod reduce;
pub mod slice;
pub mod slice_mut;
pub mod map;
pub mod weight;
pub mod zip;
pub mod range;
mod util;

#[cfg(test)]
mod test;

pub trait IntoParallelIterator {
    type Iter: ParallelIterator<Item=Self::Item>;
    type Item: Send;

    fn into_par_iter(self) -> Self::Iter;
}

pub trait IntoParallelRefIterator<'data> {
    type Iter: ParallelIterator<Item=&'data Self::Item>;
    type Item: Sync + 'data;

    fn par_iter(&'data self) -> Self::Iter;
}

pub trait IntoParallelRefMutIterator<'data> {
    type Iter: ParallelIterator<Item=&'data mut Self::Item>;
    type Item: Send + 'data;

    fn par_iter_mut(&'data mut self) -> Self::Iter;
}

/// The `ParallelIterator` interface.
pub trait ParallelIterator: Sized {
    type Item: Send;

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
    fn weight(self, scale: f64) -> Weight<Self> {
        Weight::new(self, scale)
    }

    /// Shorthand for `self.weight(f64::INFINITY)`. This forces the
    /// smallest granularity of parallel execution, which makes sense
    /// when your parallel tasks are (potentially) very expensive to
    /// execute.
    fn weight_max(self) -> Weight<Self> {
        self.weight(f64::INFINITY)
    }

    /// Executes `OP` on each item produced by the iterator, in parallel.
    fn for_each<OP>(self, op: OP)
        where OP: Fn(Self::Item) + Sync
    {
        for_each::for_each(self, &op)
    }

    /// Applies `map_op` to each item of his iterator, producing a new
    /// iterator with the results.
    fn map<MAP_OP,R>(self, map_op: MAP_OP) -> Map<Self, MAP_OP>
        where MAP_OP: Fn(Self::Item) -> R
    {
        Map::new(self, map_op)
    }

    /// Applies `map_op` to each item of his iterator, producing a new
    /// iterator with the results.
    fn filter<FILTER_OP>(self, filter_op: FILTER_OP) -> Filter<Self, FILTER_OP>
        where FILTER_OP: Fn(&Self::Item) -> bool
    {
        Filter::new(self, filter_op)
    }

    /// Applies `map_op` to each item of his iterator, producing a new
    /// iterator with the results.
    fn filter_map<FILTER_OP,R>(self, filter_op: FILTER_OP) -> FilterMap<Self, FILTER_OP>
        where FILTER_OP: Fn(Self::Item) -> Option<R>
    {
        FilterMap::new(self, filter_op)
    }

    /// Applies `map_op` to each item of his iterator, producing a new
    /// iterator with the results.
    fn flat_map<MAP_OP,PI>(self, map_op: MAP_OP) -> FlatMap<Self, MAP_OP>
        where MAP_OP: Fn(Self::Item) -> PI, PI: ParallelIterator
    {
        FlatMap::new(self, map_op)
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
        where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync,
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
        where SumOp: ReduceOp<Self::Item>
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
        where MulOp: ReduceOp<Self::Item>
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
        where MinOp: ReduceOp<Self::Item>
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
        where MaxOp: ReduceOp<Self::Item>
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
        where REDUCE_OP: ReduceOp<Self::Item>
    {
        reduce(self, reduce_op)
    }

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    #[doc(hidden)]
    fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result;
}

impl<T: ParallelIterator> IntoParallelIterator for T {
    type Iter = T;
    type Item = T::Item;

    fn into_par_iter(self) -> T {
        self
    }
}

/// A trait for parallel iterators items where the precise number of
/// items is not known, but we can at least give an upper-bound. These
/// sorts of iterators result from filtering.
///
/// # Safety note
///
/// This trait is declared as **unsafe to implement**, but it is
/// perfectly safe to **use**. It is unsafe to implement because other
/// code relies on the fact that the estimated length is an upper
/// bound in order to guarantee safety invariants.
pub unsafe trait BoundedParallelIterator: ParallelIterator {
    fn upper_bound(&mut self) -> usize;

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    #[doc(hidden)]
    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result;

}

/// A trait for parallel iterators items where the precise number of
/// items is known. This occurs when e.g. iterating over a
/// vector. Knowing precisely how many items will be produced is very
/// useful.
///
/// # Safety note
///
/// This trait is declared as **unsafe to implement**, but it is
/// perfectly safe to **use**. It is unsafe to implement because other
/// code relies on the fact that the estimated length from an
/// `ExactParallelIterator` is precisely correct in order to guarantee
/// safety invariants.
pub unsafe trait ExactParallelIterator: BoundedParallelIterator {
    /// Produces an exact count of how many items this iterator will
    /// produce, presuming no panic occurs.
    ///
    /// # Safety note
    ///
    /// Returning an incorrect value here could lead to **undefined
    /// behavior**.
    fn len(&mut self) -> usize;

    /// Collects the results of the iterator into the specified
    /// vector. The vector is always truncated before execution
    /// begins. If possible, reusing the vector across calls can lead
    /// to better performance since it reuses the same backing buffer.
    fn collect_into(self, target: &mut Vec<Self::Item>) {
        collect_into(self, target);
    }
}

/// An iterator that supports "random access" to its data, meaning
/// that you can split it at arbitrary indices and draw data from
/// those points.
pub trait IndexedParallelIterator: ExactParallelIterator {
    /// Producer type that this iterator creates. Users of the API
    /// never need to know about this type.
    #[doc(hidden)]
    type Producer: Producer<Item=Self::Item>;

    /// Internal method to convert this parallel iterator into a
    /// producer that can be used to request the items. Users of the
    /// API never need to know about this fn.
    #[doc(hidden)]
    fn into_producer(self) -> (Self::Producer, <Self::Producer as Producer>::Shared);

    /// Iterate over tuples `(A, B)`, where the items `A` are from
    /// this iterator and `B` are from the iterator given as argument.
    /// Like the `zip` method on ordinary iterators, if the two
    /// iterators are of unequal length, you only get the items they
    /// have in common.
    fn zip<ZIP_OP>(self, zip_op: ZIP_OP) -> ZipIter<Self, ZIP_OP::Iter>
        where ZIP_OP: IntoParallelIterator, ZIP_OP::Iter: IndexedParallelIterator
    {
        ZipIter::new(self, zip_op.into_par_iter())
    }

    /// Yields an index along with each item.
    fn enumerate(self) -> Enumerate<Self> {
        Enumerate::new(self)
    }
}

