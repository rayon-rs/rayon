#![allow(dead_code)]

//! The `ParallelIterator` module makes it easy to write parallel
//! programs using an iterator-style interface. To get access to all
//! the methods you want, the easiest is to write `use
//! rayon::prelude::*;` at the top of your module, which will import
//! the various traits and methods you need.
//!
//! The submodules of this module mostly just contain implementaton
//! details of little interest to an end-user. If you'd like to read
//! the code itself, the `internal` module and `README.md` file are a
//! good place to start.

use std::f64;
use std::ops::Fn;
use self::chain::ChainIter;
use self::collect::collect_into;
use self::enumerate::Enumerate;
use self::filter::Filter;
use self::filter_map::FilterMap;
use self::flat_map::FlatMap;
use self::map::{Map, MapFn, MapCloned, MapInspect};
use self::reduce::{reduce, ReduceOp, SumOp, MulOp, MinOp, MaxOp, ReduceWithOp,
                   ReduceWithIdentityOp, SUM, MUL, MIN, MAX};
use self::internal::*;
use self::threshold::Threshold;
use self::zip::ZipIter;

pub mod chain;
pub mod collect;
pub mod collections;
pub mod enumerate;
pub mod internal;
pub mod filter;
pub mod filter_map;
pub mod flat_map;
#[cfg(feature = "unstable")]
pub mod fold;
pub mod for_each;
pub mod len;
pub mod map;
pub mod option;
pub mod range;
pub mod reduce;
pub mod slice;
pub mod slice_mut;
pub mod threshold;
pub mod vec;
pub mod zip;

#[cfg(test)]
mod test;

pub trait IntoParallelIterator {
    type Iter: ParallelIterator<Item=Self::Item>;
    type Item: Send;

    fn into_par_iter(self) -> Self::Iter;
}

pub trait IntoParallelRefIterator<'data> {
    type Iter: ParallelIterator<Item=Self::Item>;
    type Item: Send + 'data;

    fn par_iter(&'data self) -> Self::Iter;
}

impl<'data, I: 'data + ?Sized> IntoParallelRefIterator<'data> for I
    where &'data I: IntoParallelIterator
{
    type Iter = <&'data I as IntoParallelIterator>::Iter;
    type Item = <&'data I as IntoParallelIterator>::Item;

    fn par_iter(&'data self) -> Self::Iter {
        self.into_par_iter()
    }
}

pub trait IntoParallelRefMutIterator<'data> {
    type Iter: ParallelIterator<Item=Self::Item>;
    type Item: Send + 'data;

    fn par_iter_mut(&'data mut self) -> Self::Iter;
}

impl<'data, I: 'data + ?Sized> IntoParallelRefMutIterator<'data> for I
    where &'data mut I: IntoParallelIterator
{
    type Iter = <&'data mut I as IntoParallelIterator>::Iter;
    type Item = <&'data mut I as IntoParallelIterator>::Item;

    fn par_iter_mut(&'data mut self) -> Self::Iter {
        self.into_par_iter()
    }
}

pub trait ToParallelChunks<'data> {
    type Iter: ParallelIterator<Item=&'data [Self::Item]>;
    type Item: Sync + 'data;

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks do not overlap.
    ///
    /// The policy for how a collection is divided into chunks is not
    /// dictated here (e.g. a B-tree-like collection may by necessity
    /// produce many chunks with fewer than `size` elements), but an
    /// implementation should strive to maximize chunk size when
    /// possible.
    fn par_chunks(&'data self, size: usize) -> Self::Iter;
}

pub trait ToParallelChunksMut<'data> {
    type Iter: ParallelIterator<Item=&'data mut [Self::Item]>;
    type Item: Send + 'data;

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks are mutable and do not overlap.
    ///
    /// The policy for how a collection is divided into chunks is not
    /// dictated here (e.g. a B-tree-like collection may by necessity
    /// produce many chunks with fewer than `size` elements), but an
    /// implementation should strive to maximize chunk size when
    /// possible.
    fn par_chunks_mut(&'data mut self, size: usize) -> Self::Iter;
}

/// The `ParallelIterator` interface.
pub trait ParallelIterator: Sized {
    type Item: Send;

    /// Fine-tune performance by suggesting a sequential cutoff
    /// threshold. This threshold defaults to 1. If you have very
    /// fine-grained work items, you may find that things run faster
    /// if you raise the threshold.  The threshold can never be less
    /// than 1.
    ///
    /// In general, if the threshold is set to some value N, then this
    /// means that if Rayon has less than N items remaining, it will
    /// not attempt to spawn another thread. So if you set the
    /// threshold to 222, and you had 400 items total, then Rayon
    /// would first split into threads, each processing 200 items, and
    /// then stop.
    ///
    /// It is recommended that you set the sequential threshold at
    /// the very end of your iterator pipeline (right before the final
    /// action). So for example:
    ///
    /// ```notest
    /// slice.par_iter()
    ///      .zip(&another_slice)
    ///      .map(some_function)
    ///      .sequential_threshold(22) // <-- here!
    ///      .for_each(...); // (final action)
    /// ```
    ///
    /// **Warning:** the threshold behavior is only a heuristic used
    /// to tune performance and should not be interpreted as any kind
    /// of guarantee. Rayon may choose not to spawn more or less
    /// threads than the threshold would suggest if it thinks this
    /// would be profitable.
    fn sequential_threshold(self, threshold: usize) -> Threshold<Self> {
        assert!(threshold >= 1, "a threshold of zero is not allowed");
        Threshold::new(self, threshold)
    }

    /// This used to be used to indicate that work items were
    /// "heavier", which would control the sequential cutoff where
    /// Rayon stopped attempting to parallelize. However, in v0.5.0,
    /// we changed the underlying model. Instead of calling weight to
    /// make your tasks seem heavier, you can call
    /// `sequential_threshold` to raise the threshold up and hence
    /// group more tasks together (it defaults to 1, meaning that we
    /// will potentially run every work item on its own thread).  Note
    /// that the sense is inverted, so if you had a big weight, you
    /// want a low sequential threshold.
    #[deprecated(since = "v0.5.0", note = "weighting model has changed; see `sequential_threshold` instead")]
    fn weight(self, _scale: f64) -> Self {
        self
    }

    /// This used to ensure that each item would (potentially) be
    /// processed on a default thread. However, that behavior is now
    /// the default. See `weight` for more background. The new method
    /// for controlling sequential cutoff is `sequential_threshold`.
    #[deprecated(since = "v0.5.0", note = "weight_max is now the default behavior")]
    fn weight_max(self) -> Self {
        self
    }

    /// Executes `OP` on each item produced by the iterator, in parallel.
    fn for_each<OP>(self, op: OP)
        where OP: Fn(Self::Item) + Sync
    {
        for_each::for_each(self, &op)
    }

    /// Applies `map_op` to each item of this iterator, producing a new
    /// iterator with the results.
    fn map<MAP_OP,R>(self, map_op: MAP_OP) -> Map<Self, MapFn<MAP_OP>>
        where MAP_OP: Fn(Self::Item) -> R + Sync
    {
        Map::new(self, MapFn(map_op))
    }

    /// Creates an iterator which clones all of its elements.  This may be
    /// useful when you have an iterator over `&T`, but you need `T`.
    fn cloned<'a, T>(self) -> Map<Self, MapCloned>
        where T: 'a + Clone, Self: ParallelIterator<Item=&'a T>
    {
        Map::new(self, MapCloned)
    }

    /// Applies `inspect_op` to a reference to each item of this iterator,
    /// producing a new iterator passing through the original items.  This is
    /// often useful for debugging to see what's happening in iterator stages.
    fn inspect<INSPECT_OP>(self, inspect_op: INSPECT_OP) -> Map<Self, MapInspect<INSPECT_OP>>
        where INSPECT_OP: Fn(&Self::Item) + Sync
    {
        Map::new(self, MapInspect(inspect_op))
    }

    /// Applies `filter_op` to each item of this iterator, producing a new
    /// iterator with only the items that gave `true` results.
    fn filter<FILTER_OP>(self, filter_op: FILTER_OP) -> Filter<Self, FILTER_OP>
        where FILTER_OP: Fn(&Self::Item) -> bool + Sync
    {
        Filter::new(self, filter_op)
    }

    /// Applies `filter_op` to each item of this iterator to get an `Option`,
    /// producing a new iterator with only the items from `Some` results.
    fn filter_map<FILTER_OP,R>(self, filter_op: FILTER_OP) -> FilterMap<Self, FILTER_OP>
        where FILTER_OP: Fn(Self::Item) -> Option<R> + Sync
    {
        FilterMap::new(self, filter_op)
    }

    /// Applies `map_op` to each item of this iterator to get nested iterators,
    /// producing a new iterator that flattens these back into one.
    fn flat_map<MAP_OP,PI>(self, map_op: MAP_OP) -> FlatMap<Self, MAP_OP>
        where MAP_OP: Fn(Self::Item) -> PI + Sync, PI: IntoParallelIterator
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
        reduce(self.map(Some), &ReduceWithOp::new(&op))
    }

    /// Reduces the items in the iterator into one item using `op`.
    /// The argument `identity` represents an "identity" value which
    /// may be inserted into the sequence as needed to create
    /// opportunities for parallel execution. So, for example, if you
    /// are doing a summation, then `identity` ought to be something
    /// that represents the zero for your type (but consider just
    /// calling `sum()` in that case).
    ///
    /// Example `vectors.par_iter().reduce_with_identity(Vector::zero(), Vector::add)`.
    ///
    /// Note: unlike in a sequential iterator, the order in which `op`
    /// will be applied to reduce the result is not specified. So `op`
    /// should be commutative and associative or else the results will
    /// be non-deterministic. And of course `identity` should be a
    /// true identity.
    fn reduce_with_identity<OP>(self, identity: Self::Item, op: OP) -> Self::Item
        where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync,
              Self::Item: Clone + Sync,
    {
        reduce(self, &ReduceWithIdentityOp::new(&identity, &op))
    }

    /// A variant on the typical `map/reduce` pattern. Parallel fold
    /// is similar to sequential fold except that the sequence of
    /// items may be subdivided before it is folded. The resulting
    /// values are then reduced together using `reduce_op`.  Typically
    /// `fold_op` and `reduce_op` will be doing the same conceptual
    /// operation, but on different types, or with a different twist.
    ///
    /// Here is how to visualize what is happening. Imagine an input
    /// sequence with 7 values as shown:
    ///
    /// ```notrust
    /// [ 0 1 2 3 4 5 6 ]
    ///   |     | |   |
    ///   +--X--+ +-Y-+ // <-- fold_op
    ///      |      |
    ///      +---Z--+   // <-- reduce_op
    /// ```
    ///
    /// These values will be first divided into contiguous chunks of
    /// some size (the precise sizes will depend on how many cores are
    /// present and how active they are). These are folded using
    /// `fold_op`. Here, the chunk `[0, 1, 2, 3]` was folded into `X`
    /// and the chunk `[4, 5, 6]` was folded into `Y`. Note that `X`
    /// and `Y` may, in general, have different types than the
    /// original input sequence. Now the results from these folds are
    /// themselves *reduced* using `reduce_op` (again, in some
    /// unspecified order). So now `X` and `Y` are reduced to `Z`,
    /// which is the final result. Note that `reduce_op` must consume
    /// and produce values of the same type.
    ///
    /// Note that `fold` can always be expressed using map/reduce. For
    /// example, a call `self.fold(identity, fold_op, reduce_op)` could
    /// also be expressed as follows:
    ///
    /// ```notest
    /// self.map(|elem| fold_op(identity.clone(), elem))
    ///     .reduce_with_identity(identity, reduce_op)
    /// ```
    ///
    /// This is equivalent to an execution of `fold` where the
    /// subsequences that were folded sequentially would up being of
    /// length 1.  However, this would rarely happen in practice,
    /// typically the subsequences would be larger, and hence a call
    /// to `fold` *can* be more efficient than map/reduce,
    /// particularly if the `fold_op` is more efficient when applied
    /// to a large sequence.
    ///
    /// **This method is marked as unstable** because it is
    /// particularly likely to change its name and/or signature, or go
    /// away entirely.
    #[cfg(feature = "unstable")]
    fn fold<I,FOLD_OP,REDUCE_OP>(self,
                                 identity: I,
                                 fold_op: FOLD_OP,
                                 reduce_op: REDUCE_OP)
                                 -> I
        where FOLD_OP: Fn(I, Self::Item) -> I + Sync,
              REDUCE_OP: Fn(I, I) -> I + Sync,
              I: Clone + Sync + Send,
    {
        fold::fold(self, &identity, &fold_op, &reduce_op)
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

    /// Takes two iterators and creates a new iterator over both.
    fn chain<CHAIN>(self, chain: CHAIN) -> ChainIter<Self, CHAIN::Iter>
        where CHAIN: IntoParallelIterator<Item=Self::Item>
    {
        ChainIter::new(self, chain.into_par_iter())
    }

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    #[doc(hidden)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>;
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
pub trait BoundedParallelIterator: ParallelIterator {
    fn upper_bound(&mut self) -> usize;

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    #[doc(hidden)]
    fn drive<'c, C: Consumer<Self::Item>>(self,
                                                   consumer: C)
                                                   -> C::Result;

}

/// A trait for parallel iterators items where the precise number of
/// items is known. This occurs when e.g. iterating over a
/// vector. Knowing precisely how many items will be produced is very
/// useful.
pub trait ExactParallelIterator: BoundedParallelIterator {
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
    /// Internal method to convert this parallel iterator into a
    /// producer that can be used to request the items. Users of the
    /// API never need to know about this fn.
    #[doc(hidden)]
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output;

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

