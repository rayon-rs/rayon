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
use std::cmp::{self, Ordering};
use std::ops::Fn;
use self::chain::ChainIter;
use self::collect::collect_into;
use self::enumerate::Enumerate;
use self::filter::Filter;
use self::filter_map::FilterMap;
use self::flat_map::FlatMap;
use self::from_par_iter::FromParallelIterator;
use self::map::{Map, MapFn, MapCloned, MapInspect};
use self::reduce::{reduce, ReduceOp, SumOp, ProductOp, ReduceWithIdentityOp, SUM, PRODUCT};
use self::skip::Skip;
use self::take::Take;
use self::internal::*;
use self::weight::Weight;
use self::zip::ZipIter;

pub mod find;
pub mod chain;
pub mod collect;
pub mod enumerate;
pub mod filter;
pub mod filter_map;
pub mod flat_map;
pub mod from_par_iter;
pub mod internal;
pub mod len;
pub mod for_each;
pub mod fold;
pub mod reduce;
pub mod skip;
pub mod take;
pub mod slice;
pub mod slice_mut;
pub mod string;
pub mod map;
pub mod weight;
pub mod zip;
pub mod range;
pub mod vec;
pub mod option;
pub mod collections;
pub mod noop;

#[cfg(test)]
mod test;

pub trait IntoParallelIterator {
    type Iter: ParallelIterator<Item = Self::Item>;
    type Item: Send;

    fn into_par_iter(self) -> Self::Iter;
}

pub trait IntoParallelRefIterator<'data> {
    type Iter: ParallelIterator<Item = Self::Item>;
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
    type Iter: ParallelIterator<Item = Self::Item>;
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
    type Iter: ParallelIterator<Item = &'data [Self::Item]>;
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
    type Iter: ParallelIterator<Item = &'data mut [Self::Item]>;
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

/// Parallel extensions for strings.
pub trait ParallelString {
    type Chars;

    /// Returns a parallel iterator over the characters of a string.
    fn par_chars(self) -> Self::Chars;
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

    /// Counts the number of items in this parallel iterator.
    fn count(self) -> usize {
        self.map(|_| 1).sum()
    }

    /// Applies `map_op` to each item of this iterator, producing a new
    /// iterator with the results.
    fn map<MAP_OP, R>(self, map_op: MAP_OP) -> Map<Self, MapFn<MAP_OP>>
        where MAP_OP: Fn(Self::Item) -> R + Sync
    {
        Map::new(self, MapFn(map_op))
    }

    /// Creates an iterator which clones all of its elements.  This may be
    /// useful when you have an iterator over `&T`, but you need `T`.
    fn cloned<'a, T>(self) -> Map<Self, MapCloned>
        where T: 'a + Clone,
              Self: ParallelIterator<Item = &'a T>
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
    fn filter_map<FILTER_OP, R>(self, filter_op: FILTER_OP) -> FilterMap<Self, FILTER_OP>
        where FILTER_OP: Fn(Self::Item) -> Option<R> + Sync
    {
        FilterMap::new(self, filter_op)
    }

    /// Applies `map_op` to each item of this iterator to get nested iterators,
    /// producing a new iterator that flattens these back into one.
    fn flat_map<MAP_OP, PI>(self, map_op: MAP_OP) -> FlatMap<Self, MAP_OP>
        where MAP_OP: Fn(Self::Item) -> PI + Sync,
              PI: IntoParallelIterator
    {
        FlatMap::new(self, map_op)
    }

    /// Reduces the items in the iterator into one item using `op`.
    /// The argument `identity` should be a closure that can produce
    /// "identity" value which may be inserted into the sequence as
    /// needed to create opportunities for parallel execution. So, for
    /// example, if you are doing a summation, then `identity()` ought
    /// to produce something that represents the zero for your type
    /// (but consider just calling `sum()` in that case).
    ///
    /// Example:
    ///
    /// ```
    /// // Iterate over a sequence of pairs `(x0, y0), ..., (xN, yN)`
    /// // and use reduce to compute one pair `(x0 + ... + xN, y0 + ... + yN)`
    /// // where the first/second elements are summed separately.
    /// use rayon::prelude::*;
    /// let sums = [(0, 1), (5, 6), (16, 2), (8, 9)]
    ///            .par_iter()        // iterating over &(i32, i32)
    ///            .cloned()          // iterating over (i32, i32)
    ///            .reduce(|| (0, 0), // the "identity" is 0 in both columns
    ///                    |a, b| (a.0 + b.0, a.1 + b.1));
    /// assert_eq!(sums, (0 + 5 + 16 + 8, 1 + 6 + 2 + 9));
    /// ```
    ///
    /// **Note:** unlike a sequential `fold` operation, the order in
    /// which `op` will be applied to reduce the result is not fully
    /// specified. So `op` should be [associative] or else the results
    /// will be non-deterministic. And of course `identity()` should
    /// produce a true identity.
    ///
    /// [associative]: https://en.wikipedia.org/wiki/Associative_property
    fn reduce<OP, IDENTITY>(self, identity: IDENTITY, op: OP) -> Self::Item
        where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync,
              IDENTITY: Fn() -> Self::Item + Sync
    {
        reduce(self, &ReduceWithIdentityOp::new(&identity, &op))
    }

    /// Reduces the items in the iterator into one item using `op`.
    /// If the iterator is empty, `None` is returned; otherwise,
    /// `Some` is returned.
    ///
    /// This version of `reduce` is simple but somewhat less
    /// efficient. If possible, it is better to call `reduce()`, which
    /// requires an identity element.
    ///
    /// **Note:** unlike a sequential `fold` operation, the order in
    /// which `op` will be applied to reduce the result is not fully
    /// specified. So `op` should be [associative] or else the results
    /// will be non-deterministic.
    ///
    /// [associative]: https://en.wikipedia.org/wiki/Associative_property
    fn reduce_with<OP>(self, op: OP) -> Option<Self::Item>
        where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync
    {
        self.map(Some)
            .reduce(|| None, |opt_a, opt_b| match (opt_a, opt_b) {
                (Some(a), Some(b)) => Some(op(a, b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            })
    }

    /// Deprecated. Use `reduce()` instead.
    #[deprecated(since = "v0.5.0", note = "call `reduce` instead")]
    fn reduce_with_identity<OP>(self, identity: Self::Item, op: OP) -> Self::Item
        where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync,
              Self::Item: Clone + Sync
    {
        self.reduce(|| identity.clone(), op)
    }

    /// Parallel fold is similar to sequential fold except that the
    /// sequence of items may be subdivided before it is
    /// folded. Consider a list of numbers like `22 3 77 89 46`. If
    /// you used sequential fold to add them (`fold(0, |a,b| a+b)`,
    /// you would wind up first adding 0 + 22, then 22 + 3, then 25 +
    /// 77, and so forth. The **parallel fold** works similarly except
    /// that it first breaks up your list into sublists, and hence
    /// instead of yielding up a single sum at the end, it yields up
    /// multiple sums. The number of results is nondeterministic, as
    /// is the point where the breaks occur.
    ///
    /// So if did the same parallel fold (`fold(0, |a,b| a+b)`) on
    /// our example list, we might wind up with a sequence of two numbers,
    /// like so:
    ///
    /// ```notrust
    /// 22 3 77 89 46
    ///       |     |
    ///     102   135
    /// ```
    ///
    /// Or perhaps these three numbers:
    ///
    /// ```notrust
    /// 22 3 77 89 46
    ///       |  |  |
    ///     102 89 46
    /// ```
    ///
    /// In general, Rayon will attempt to find good breaking points
    /// that keep all of your cores busy.
    ///
    /// ### Fold versus reduce
    ///
    /// The `fold()` and `reduce()` methods each take an identity element
    /// and a combining function, but they operate rather differently.
    ///
    /// `reduce()` requires that the identity function has the same
    /// type as the things you are iterating over, and it fully
    /// reduces the list of items into a single item. So, for example,
    /// imagine we are iterating over a list of bytes `bytes: [128_u8,
    /// 64_u8, 64_u8]`. If we used `bytes.reduce(|| 0_u8, |a: u8, b:
    /// u8| a + b)`, we would get an overflow. This is because `0`,
    /// `a`, and `b` here are all bytes, just like the numbers in the
    /// list (I wrote the types explicitly above, but those are the
    /// only types you can use). To avoid the overflow, we would need
    /// to do something like `bytes.map(|b| b as u32).reduce(|| 0, |a,
    /// b| a + b)`, in which case our result would be `256`.
    ///
    /// In contrast, with `fold()`, the identity function does not
    /// have to have the same type as the things you are iterating
    /// over, and you potentially get back many results. So, if we
    /// continue with the `bytes` example from the previous paragraph,
    /// we could do `bytes.fold(|| 0_u32, |a, b| a + (b as u32))` to
    /// convert our bytes into `u32`. And of course we might not get
    /// back a single sum.
    ///
    /// There is a more subtle distinction as well, though it's
    /// actually implied by the above points. When you use `reduce()`,
    /// your reduction function is sometimes called with values that
    /// were never part of your original parallel iterator (for
    /// example, both the left and right might be a partial sum). With
    /// `fold()`, in contrast, the left value in the fold function is
    /// always the accumulator, and the right value is always from
    /// your original sequence.
    ///
    /// ### Fold vs Map/Reduce
    ///
    /// Fold makes sense if you have some operation where it is
    /// cheaper to groups of elements at a time. For example, imagine
    /// collecting characters into a string. If you were going to use
    /// map/reduce, you might try this:
    ///
    /// ```
    /// use rayon::prelude::*;
    /// let s =
    ///     ['a', 'b', 'c', 'd', 'e']
    ///     .par_iter()
    ///     .map(|c: &char| format!("{}", c))
    ///     .reduce(|| String::new(),
    ///             |mut a: String, b: String| { a.push_str(&b); a });
    /// assert_eq!(s, "abcde");
    /// ```
    ///
    /// Because reduce produces the same type of element as its input,
    /// you have to first map each character into a string, and then
    /// you can reduce them. This means we create one string per
    /// element in ou iterator -- not so great. Using `fold`, we can
    /// do this instead:
    ///
    /// ```
    /// use rayon::prelude::*;
    /// let s =
    ///     ['a', 'b', 'c', 'd', 'e']
    ///     .par_iter()
    ///     .fold(|| String::new(),
    ///             |mut s: String, c: &char| { s.push(*c); s })
    ///     .reduce(|| String::new(),
    ///             |mut a: String, b: String| { a.push_str(&b); a });
    /// assert_eq!(s, "abcde");
    /// ```
    ///
    /// Now `fold` will process groups of our characters at a time,
    /// and we only make one string per group. We should wind up with
    /// some small-ish number of strings roughly proportional to the
    /// number of CPUs you have (it will ultimately depend on how busy
    /// your processors are). Note that we still need to do a reduce
    /// afterwards to combine those groups of strings into a single
    /// string.
    ///
    /// You could use a similar trick to save partial results (e.g., a
    /// cache) or something similar.
    ///
    /// ### Combining fold with other operations
    ///
    /// You can combine `fold` with `reduce` if you want to produce a
    /// single value. This is then roughly equivalent to a map/reduce
    /// combination in effect:
    ///
    /// ```
    /// use rayon::prelude::*;
    /// let bytes = 0..22_u8; // series of u8 bytes
    /// let sum = bytes.into_par_iter()
    ///                .fold(|| 0_u32, |a: u32, b: u8| a + (b as u32))
    ///                .sum();
    /// assert_eq!(sum, (0..22).sum()); // compare to sequential
    /// ```
    fn fold<IDENTITY_ITEM, IDENTITY, FOLD_OP>(self,
                                              identity: IDENTITY,
                                              fold_op: FOLD_OP)
                                              -> fold::Fold<Self, IDENTITY, FOLD_OP>
        where FOLD_OP: Fn(IDENTITY_ITEM, Self::Item) -> IDENTITY_ITEM + Sync,
              IDENTITY: Fn() -> IDENTITY_ITEM + Sync,
              IDENTITY_ITEM: Send
    {
        fold::fold(self, identity, fold_op)
    }

    /// Sums up the items in the iterator.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `+` operator is not truly [associative] (as is the
    /// case for floating point numbers), then the results are not
    /// fully deterministic.
    ///
    /// [associative]: https://en.wikipedia.org/wiki/Associative_property
    ///
    /// Basically equivalent to `self.reduce(|| 0, |a, b| a + b)`,
    /// except that the type of `0` and the `+` operation may vary
    /// depending on the type of value being produced.
    fn sum(self) -> Self::Item
        where SumOp: ReduceOp<Self::Item>
    {
        reduce(self, SUM)
    }

    /// Multiplies all the items in the iterator.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `*` operator is not truly [associative] (as is the
    /// case for floating point numbers), then the results are not
    /// fully deterministic.
    ///
    /// [associative]: https://en.wikipedia.org/wiki/Associative_property
    ///
    /// Basically equivalent to `self.reduce(|| 1, |a, b| a * b)`,
    /// except that the type of `1` and the `*` operation may vary
    /// depending on the type of value being produced.
    fn product(self) -> Self::Item
        where ProductOp: ReduceOp<Self::Item>
    {
        reduce(self, PRODUCT)
    }

    /// DEPRECATED
    #[deprecated(since = "v0.6.0",
        note = "name changed to `product()` to match sequential iterators")]
    fn mul(self) -> Self::Item
        where ProductOp: ReduceOp<Self::Item>
    {
        reduce(self, PRODUCT)
    }

    /// Computes the minimum of all the items in the iterator. If the
    /// iterator is empty, `None` is returned; otherwise, `Some(min)`
    /// is returned.
    ///
    /// Note that the order in which the items will be reduced is not
    /// specified, so if the `Ord` impl is not truly associative, then
    /// the results are not deterministic.
    ///
    /// Basically equivalent to `self.reduce_with(|a, b| cmp::min(a, b))`.
    fn min(self) -> Option<Self::Item>
        where Self::Item: Ord
    {
        self.reduce_with(cmp::min)
    }

    /// Computes the item that yields the minimum value for the given
    /// function. If the iterator is empty, `None` is returned;
    /// otherwise, `Some(item)` is returned.
    ///
    /// Note that the order in which the items will be reduced is not
    /// specified, so if the `Ord` impl is not truly associative, then
    /// the results are not deterministic.
    fn min_by_key<K, F>(self, f: F) -> Option<Self::Item>
        where K: Ord + Send,
              F: Sync + Fn(&Self::Item) -> K
    {
        self.map(|x| (f(&x), x))
            .reduce_with(|a, b| match (a.0).cmp(&b.0) {
                Ordering::Greater => b,
                _ => a,
            })
            .map(|(_, x)| x)
    }

    /// Computes the maximum of all the items in the iterator. If the
    /// iterator is empty, `None` is returned; otherwise, `Some(max)`
    /// is returned.
    ///
    /// Note that the order in which the items will be reduced is not
    /// specified, so if the `Ord` impl is not truly associative, then
    /// the results are not deterministic.
    ///
    /// Basically equivalent to `self.reduce_with(|a, b| cmp::max(a, b))`.
    fn max(self) -> Option<Self::Item>
        where Self::Item: Ord
    {
        self.reduce_with(cmp::max)
    }

    /// Computes the item that yields the maximum value for the given
    /// function. If the iterator is empty, `None` is returned;
    /// otherwise, `Some(item)` is returned.
    ///
    /// Note that the order in which the items will be reduced is not
    /// specified, so if the `Ord` impl is not truly associative, then
    /// the results are not deterministic.
    fn max_by_key<K, F>(self, f: F) -> Option<Self::Item>
        where K: Ord + Send,
              F: Sync + Fn(&Self::Item) -> K
    {
        self.map(|x| (f(&x), x))
            .reduce_with(|a, b| match (a.0).cmp(&b.0) {
                Ordering::Greater => a,
                _ => b,
            })
            .map(|(_, x)| x)
    }

    /// Takes two iterators and creates a new iterator over both.
    fn chain<CHAIN>(self, chain: CHAIN) -> ChainIter<Self, CHAIN::Iter>
        where CHAIN: IntoParallelIterator<Item = Self::Item>
    {
        ChainIter::new(self, chain.into_par_iter())
    }

    /// Searches for **some** item in the parallel iterator that
    /// matches the given predicate and returns it. This operation
    /// is similar to [`find` on sequential iterators][find] but
    /// the item returned may not be the **first** one in the parallel
    /// sequence which matches, since we search the entire sequence in parallel.
    ///
    /// Once a match is found, we will attempt to stop processing
    /// the rest of the items in the iterator as soon as possible
    /// (just as `find` stops iterating once a match is found).
    ///
    /// [find]: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.find
    fn find_any<FIND_OP>(self, predicate: FIND_OP) -> Option<Self::Item>
        where FIND_OP: Fn(&Self::Item) -> bool + Sync
    {
        find::find(self, predicate)
    }

    #[doc(hidden)]
    #[deprecated(note = "parallel `find` does not search in order -- use `find_any`")]
    fn find<FIND_OP>(self, predicate: FIND_OP) -> Option<Self::Item>
        where FIND_OP: Fn(&Self::Item) -> bool + Sync
    {
        self.find_any(predicate)
    }

    /// Searches for **some** item in the parallel iterator that
    /// matches the given predicate, and if so returns true.  Once
    /// a match is found, we'll attempt to stop process the rest
    /// of the items.  Proving that there's no match, returning false,
    /// does require visiting every item.
    fn any<ANY_OP>(self, predicate: ANY_OP) -> bool
        where ANY_OP: Fn(Self::Item) -> bool + Sync
    {
        self.map(predicate).find_any(|&p| p).is_some()
    }

    /// Tests that every item in the parallel iterator matches the given
    /// predicate, and if so returns true.  If a counter-example is found,
    /// we'll attempt to stop processing more items, then return false.
    fn all<ALL_OP>(self, predicate: ALL_OP) -> bool
        where ALL_OP: Fn(Self::Item) -> bool + Sync
    {
        self.map(predicate).find_any(|&p| !p).is_none()
    }

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    #[doc(hidden)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result where C: UnindexedConsumer<Self::Item>;

    /// Returns the number of items produced by this iterator, if known
    /// statically. This can be used by consumers to trigger special fast
    /// paths. Therefore, if `Some(_)` is returned, this iterator must only
    /// use the (indexed) `Consumer` methods when driving a consumer, such
    /// as `split_at()`. Calling `UnindexedConsumer::split_off()` or other
    /// `UnindexedConsumer` methods -- or returning an inaccurate value --
    /// may result in panics.
    ///
    /// This is hidden & considered internal for now, until we decide
    /// whether it makes sense for a public API.  Right now it is only used
    /// to optimize `collect`  for want of true Rust specialization.
    #[doc(hidden)]
    fn opt_len(&mut self) -> Option<usize> {
        None
    }

    /// Create a fresh collection containing all the element produced
    /// by this parallel iterator.
    ///
    /// You may prefer to use `collect_into()`, which allocates more
    /// efficiently with precise knowledge of how many elements the
    /// iterator contains, and even allows you to reuse an existing
    /// vector's backing store rather than allocating a fresh vector.
    fn collect<C>(self) -> C
        where C: FromParallelIterator<Self::Item>
    {
        C::from_par_iter(self)
    }
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
    fn drive<'c, C: Consumer<Self::Item>>(self, consumer: C) -> C::Result;
}

/// A trait for parallel iterators items where the precise number of
/// items is known. This occurs when e.g. iterating over a
/// vector. Knowing precisely how many items will be produced is very
/// useful.
pub trait ExactParallelIterator: BoundedParallelIterator {
    /// Produces an exact count of how many items this iterator will
    /// produce, presuming no panic occurs.
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
        where ZIP_OP: IntoParallelIterator,
              ZIP_OP::Iter: IndexedParallelIterator
    {
        ZipIter::new(self, zip_op.into_par_iter())
    }

    /// Lexicographically compares the elements of this `ParallelIterator` with those of
    /// another.
    fn cmp<I>(self, other: I) -> Ordering
        where I: IntoParallelIterator<Item = Self::Item>,
              I::Iter: IndexedParallelIterator,
              Self::Item: Ord
    {
        self.zip(other.into_par_iter())
            .map(|(x, y)| Ord::cmp(&x, &y))
            .reduce(|| Ordering::Equal,
                    |cmp_a, cmp_b| if cmp_a == Ordering::Equal {
                        cmp_b
                    } else {
                        cmp_a
                    })
    }

    /// Lexicographically compares the elements of this `ParallelIterator` with those of
    /// another.
    fn partial_cmp<I>(self, other: I) -> Option<Ordering>
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialOrd<I::Item>
    {
        self.zip(other.into_par_iter())
            .map(|(x, y)| PartialOrd::partial_cmp(&x, &y))
            .reduce(|| Some(Ordering::Equal), |cmp_a, cmp_b| match cmp_a {
                Some(Ordering::Equal) => cmp_b,
                Some(_) => cmp_a,
                None => None,
            })
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are equal to those of another
    fn eq<I>(self, other: I) -> bool
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialEq<I::Item>
    {
        self.zip(other.into_par_iter())
            .all(|(x, y)| x.eq(&y))
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are unequal to those of another
    fn ne<I>(self, other: I) -> bool
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialEq<I::Item>
    {
        !self.eq(other)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are lexicographically less than those of another.
    fn lt<I>(self, other: I) -> bool
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialOrd<I::Item>
    {
        self.partial_cmp(other) == Some(Ordering::Less)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are less or equal to those of another.
    fn le<I>(self, other: I) -> bool
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialOrd<I::Item>
    {
        let ord = self.partial_cmp(other);
        ord == Some(Ordering::Equal) || ord == Some(Ordering::Less)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are lexicographically greater than those of another.
    fn gt<I>(self, other: I) -> bool
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialOrd<I::Item>
    {
        self.partial_cmp(other) == Some(Ordering::Greater)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are less or equal to those of another.
    fn ge<I>(self, other: I) -> bool
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialOrd<I::Item>
    {
        let ord = self.partial_cmp(other);
        ord == Some(Ordering::Equal) || ord == Some(Ordering::Greater)
    }

    /// Yields an index along with each item.
    fn enumerate(self) -> Enumerate<Self> {
        Enumerate::new(self)
    }

    /// Creates an iterator that skips the first `n` elements.
    fn skip(self, n: usize) -> Skip<Self> {
        Skip::new(self, n)
    }

    /// Creates an iterator that yields the first `n` elements.
    fn take(self, n: usize) -> Take<Self> {
        Take::new(self, n)
    }

    /// Searches for **some** item in the parallel iterator that
    /// matches the given predicate, and returns its index.  Like
    /// `ParallelIterator::find_any`, the parallel search will not
    /// necessarily find the **first** match, and once a match is
    /// found we'll attempt to stop processing any more.
    fn position_any<POSITION_OP>(self, predicate: POSITION_OP) -> Option<usize>
        where POSITION_OP: Fn(Self::Item) -> bool + Sync
    {
        self.map(predicate)
            .enumerate()
            .find_any(|&(_, p)| p)
            .map(|(i, _)| i)
    }

    #[doc(hidden)]
    #[deprecated(note = "parallel `position` does not search in order -- use `position_any`")]
    fn position<POSITION_OP>(self, predicate: POSITION_OP) -> Option<usize>
        where POSITION_OP: Fn(Self::Item) -> bool + Sync
    {
        self.position_any(predicate)
    }
}
