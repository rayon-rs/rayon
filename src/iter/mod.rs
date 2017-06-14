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

use std::cmp::{self, Ordering};
use std::iter::{Sum, Product};
use std::ops::Fn;
use self::internal::*;

// There is a method to the madness here:
//
// - Most of these modules are private but expose certain types to the end-user
//   (e.g., `enumerate::Enumerate`) -- specifically, the types that appear in the
//   public API surface of the `ParallelIterator` traits.
// - In **this** module, those public types are always used unprefixed, which forces
//   us to add a `pub use` and helps identify if we missed anything.
// - In contrast, items that appear **only** in the body of a method,
//   e.g. `find::find()`, are always used **prefixed**, so that they
//   can be readily distinguished.

mod find;
mod find_first_last;
mod chain;
pub use self::chain::Chain;
mod collect;
mod enumerate;
pub use self::enumerate::Enumerate;
mod filter;
pub use self::filter::Filter;
mod filter_map;
pub use self::filter_map::FilterMap;
mod flat_map;
pub use self::flat_map::FlatMap;
mod from_par_iter;
pub mod internal;
mod for_each;
mod fold;
pub use self::fold::{Fold, FoldWith};
mod reduce;
mod skip;
pub use self::skip::Skip;
mod splitter;
pub use self::splitter::{split, Split};
mod take;
pub use self::take::Take;
mod map;
pub use self::map::Map;
mod map_with;
pub use self::map_with::MapWith;
mod zip;
pub use self::zip::Zip;
mod noop;
mod rev;
pub use self::rev::Rev;
mod len;
pub use self::len::{MinLen, MaxLen};
mod sum;
mod product;
mod cloned;
pub use self::cloned::Cloned;
mod inspect;
pub use self::inspect::Inspect;
mod while_some;
pub use self::while_some::WhileSome;
mod extend;
mod unzip;

#[cfg(test)]
mod test;

/// Represents a value of one of two possible types.
pub enum Either<L, R> {
    Left(L),
    Right(R)
}

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

/// The `ParallelIterator` interface.
pub trait ParallelIterator: Sized + Send {
    type Item: Send;

    /// Executes `OP` on each item produced by the iterator, in parallel.
    fn for_each<OP>(self, op: OP)
        where OP: Fn(Self::Item) + Sync + Send
    {
        for_each::for_each(self, &op)
    }

    /// Executes `OP` on the given `init` value with each item produced by
    /// the iterator, in parallel.
    ///
    /// The `init` value will be cloned only as needed to be paired with
    /// the group of items in each rayon job.  It does not require the type
    /// to be `Sync`.
    fn for_each_with<OP, T>(self, init: T, op: OP)
        where OP: Fn(&mut T, Self::Item) + Sync + Send,
              T: Send + Clone
    {
        self.map_with(init, op).for_each(|()| ())
    }

    /// Counts the number of items in this parallel iterator.
    fn count(self) -> usize {
        self.map(|_| 1).sum()
    }

    /// Applies `map_op` to each item of this iterator, producing a new
    /// iterator with the results.
    fn map<F, R>(self, map_op: F) -> Map<Self, F>
        where F: Fn(Self::Item) -> R + Sync + Send,
              R: Send
    {
        map::new(self, map_op)
    }

    /// Applies `map_op` to the given `init` value with each item of this
    /// iterator, producing a new iterator with the results.
    ///
    /// The `init` value will be cloned only as needed to be paired with
    /// the group of items in each rayon job.  It does not require the type
    /// to be `Sync`.
    fn map_with<F, T, R>(self, init: T, map_op: F) -> MapWith<Self, T, F>
        where F: Fn(&mut T, Self::Item) -> R + Sync + Send,
              T: Send + Clone,
              R: Send
    {
        map_with::new(self, init, map_op)
    }

    /// Creates an iterator which clones all of its elements.  This may be
    /// useful when you have an iterator over `&T`, but you need `T`.
    fn cloned<'a, T>(self) -> Cloned<Self>
        where T: 'a + Clone + Send,
              Self: ParallelIterator<Item = &'a T>
    {
        cloned::new(self)
    }

    /// Applies `inspect_op` to a reference to each item of this iterator,
    /// producing a new iterator passing through the original items.  This is
    /// often useful for debugging to see what's happening in iterator stages.
    fn inspect<OP>(self, inspect_op: OP) -> Inspect<Self, OP>
        where OP: Fn(&Self::Item) + Sync + Send
    {
        inspect::new(self, inspect_op)
    }

    /// Applies `filter_op` to each item of this iterator, producing a new
    /// iterator with only the items that gave `true` results.
    fn filter<P>(self, filter_op: P) -> Filter<Self, P>
        where P: Fn(&Self::Item) -> bool + Sync + Send
    {
        filter::new(self, filter_op)
    }

    /// Applies `filter_op` to each item of this iterator to get an `Option`,
    /// producing a new iterator with only the items from `Some` results.
    fn filter_map<P, R>(self, filter_op: P) -> FilterMap<Self, P>
        where P: Fn(Self::Item) -> Option<R> + Sync + Send,
              R: Send
    {
        filter_map::new(self, filter_op)
    }

    /// Applies `map_op` to each item of this iterator to get nested iterators,
    /// producing a new iterator that flattens these back into one.
    fn flat_map<F, PI>(self, map_op: F) -> FlatMap<Self, F>
        where F: Fn(Self::Item) -> PI + Sync + Send,
              PI: IntoParallelIterator
    {
        flat_map::new(self, map_op)
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
    fn reduce<OP, ID>(self, identity: ID, op: OP) -> Self::Item
        where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync + Send,
              ID: Fn() -> Self::Item + Sync + Send
    {
        reduce::reduce(self, identity, op)
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
        where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync + Send
    {
        self.fold(|| None, |opt_a, b| match opt_a {
                Some(a) => Some(op(a, b)),
                None => Some(b),
            })
            .reduce(|| None, |opt_a, opt_b| match (opt_a, opt_b) {
                (Some(a), Some(b)) => Some(op(a, b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            })
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
    ///                .sum::<u32>();
    /// assert_eq!(sum, (0..22).sum()); // compare to sequential
    /// ```
    fn fold<T, ID, F>(self, identity: ID, fold_op: F) -> Fold<Self, ID, F>
        where F: Fn(T, Self::Item) -> T + Sync + Send,
              ID: Fn() -> T + Sync + Send,
              T: Send
    {
        fold::fold(self, identity, fold_op)
    }

    /// Applies `fold_op` to the given `init` value with each item of this
    /// iterator, finally producing the value for further use.
    ///
    /// This works essentially like `fold(|| init.clone(), fold_op)`, except
    /// it doesn't require the `init` type to be `Sync`, nor any other form
    /// of added synchronization.
    fn fold_with<F, T>(self, init: T, fold_op: F) -> FoldWith<Self, T, F>
        where F: Fn(T, Self::Item) -> T + Sync + Send,
              T: Send + Clone
    {
        fold::fold_with(self, init, fold_op)
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
    fn sum<S>(self) -> S
        where S: Send + Sum<Self::Item> + Sum
    {
        sum::sum(self)
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
    fn product<P>(self) -> P
        where P: Send + Product<Self::Item> + Product
    {
        product::product(self)
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

    /// Computes the minimum of all the items in the iterator with respect to
    /// the given comparison function. If the iterator is empty, `None` is
    /// returned; otherwise, `Some(min)` is returned.
    ///
    /// Note that the order in which the items will be reduced is not
    /// specified, so if the comparison function is not associative, then
    /// the results are not deterministic.
    fn min_by<F>(self, f: F) -> Option<Self::Item>
        where F: Sync + Send + Fn(&Self::Item, &Self::Item) -> Ordering
    {
        self.reduce_with(|a, b| match f(&a, &b) {
                             Ordering::Greater => b,
                             _ => a,
                         })
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
              F: Sync + Send + Fn(&Self::Item) -> K
    {
        self.map(|x| (f(&x), x))
            .min_by(|a, b| (a.0).cmp(&b.0))
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

    /// Computes the maximum of all the items in the iterator with respect to
    /// the given comparison function. If the iterator is empty, `None` is
    /// returned; otherwise, `Some(min)` is returned.
    ///
    /// Note that the order in which the items will be reduced is not
    /// specified, so if the comparison function is not associative, then
    /// the results are not deterministic.
    fn max_by<F>(self, f: F) -> Option<Self::Item>
        where F: Sync + Send + Fn(&Self::Item, &Self::Item) -> Ordering
    {
        self.reduce_with(|a, b| match f(&a, &b) {
                             Ordering::Greater => a,
                             _ => b,
                         })
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
              F: Sync + Send + Fn(&Self::Item) -> K
    {
        self.map(|x| (f(&x), x))
            .max_by(|a, b| (a.0).cmp(&b.0))
            .map(|(_, x)| x)
    }

    /// Takes two iterators and creates a new iterator over both.
    fn chain<C>(self, chain: C) -> Chain<Self, C::Iter>
        where C: IntoParallelIterator<Item = Self::Item>
    {
        chain::new(self, chain.into_par_iter())
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
    fn find_any<P>(self, predicate: P) -> Option<Self::Item>
        where P: Fn(&Self::Item) -> bool + Sync + Send
    {
        find::find(self, predicate)
    }

    /// Searches for the sequentially **first** item in the parallel iterator
    /// that matches the given predicate and returns it.
    ///
    /// Once a match is found, all attempts to the right of the match
    /// will be stopped, while attempts to the left must continue in case
    /// an earlier match is found.
    ///
    /// Note that not all parallel iterators have a useful order, much like
    /// sequential `HashMap` iteration, so "first" may be nebulous.  If you
    /// just want the first match that discovered anywhere in the iterator,
    /// `find_any` is a better choice.
    fn find_first<P>(self, predicate: P) -> Option<Self::Item>
        where P: Fn(&Self::Item) -> bool + Sync + Send
    {
        find_first_last::find_first(self, predicate)
    }

    /// Searches for the sequentially **last** item in the parallel iterator
    /// that matches the given predicate and returns it.
    ///
    /// Once a match is found, all attempts to the left of the match
    /// will be stopped, while attempts to the right must continue in case
    /// a later match is found.
    ///
    /// Note that not all parallel iterators have a useful order, much like
    /// sequential `HashMap` iteration, so "last" may be nebulous.  When the
    /// order doesn't actually matter to you, `find_any` is a better choice.
    fn find_last<P>(self, predicate: P) -> Option<Self::Item>
        where P: Fn(&Self::Item) -> bool + Sync + Send
    {
        find_first_last::find_last(self, predicate)
    }

    #[doc(hidden)]
    #[deprecated(note = "parallel `find` does not search in order -- use `find_any`, \\
    `find_first`, or `find_last`")]
    fn find<P>(self, predicate: P) -> Option<Self::Item>
        where P: Fn(&Self::Item) -> bool + Sync + Send
    {
        self.find_any(predicate)
    }

    /// Searches for **some** item in the parallel iterator that
    /// matches the given predicate, and if so returns true.  Once
    /// a match is found, we'll attempt to stop process the rest
    /// of the items.  Proving that there's no match, returning false,
    /// does require visiting every item.
    fn any<P>(self, predicate: P) -> bool
        where P: Fn(Self::Item) -> bool + Sync + Send
    {
        self.map(predicate).find_any(|&p| p).is_some()
    }

    /// Tests that every item in the parallel iterator matches the given
    /// predicate, and if so returns true.  If a counter-example is found,
    /// we'll attempt to stop processing more items, then return false.
    fn all<P>(self, predicate: P) -> bool
        where P: Fn(Self::Item) -> bool + Sync + Send
    {
        self.map(predicate).find_any(|&p| !p).is_none()
    }

    /// Creates an iterator over the `Some` items of this iterator, halting
    /// as soon as any `None` is found.
    fn while_some<T>(self) -> WhileSome<Self>
        where Self: ParallelIterator<Item = Option<T>>,
              T: Send
    {
        while_some::new(self)
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

    /// Unzips the items of a parallel iterator into a pair of arbitrary
    /// `ParallelExtend` containers.
    ///
    /// You may prefer to use `unzip_into()`, which allocates more
    /// efficiently with precise knowledge of how many elements the
    /// iterator contains, and even allows you to reuse existing
    /// vectors' backing stores rather than allocating fresh vectors.
    fn unzip<A, B, FromA, FromB>(self) -> (FromA, FromB)
        where Self: ParallelIterator<Item = (A, B)>,
              FromA: Default + Send + ParallelExtend<A>,
              FromB: Default + Send + ParallelExtend<B>,
              A: Send,
              B: Send
    {
        unzip::unzip(self)
    }

    /// Partitions the items of a parallel iterator into a pair of arbitrary
    /// `ParallelExtend` containers.  Items for which the `predicate` returns
    /// true go into the first container, and the rest go into the second.
    ///
    /// Note: unlike the standard `Iterator::partition`, this allows distinct
    /// collection types for the left and right items.  This is more flexible,
    /// but may require new type annotations when converting sequential code
    /// that used type inferrence assuming the two were the same.
    fn partition<A, B, P>(self, predicate: P) -> (A, B)
        where A: Default + Send + ParallelExtend<Self::Item>,
              B: Default + Send + ParallelExtend<Self::Item>,
              P: Fn(&Self::Item) -> bool + Sync + Send
    {
        unzip::partition(self, predicate)
    }

    /// Partitions and maps the items of a parallel iterator into a pair of
    /// arbitrary `ParallelExtend` containers.  `Either::Left` items go into
    /// the first container, and `Either::Right` items go into the second.
    fn partition_map<A, B, P, L, R>(self, predicate: P) -> (A, B)
        where A: Default + Send + ParallelExtend<L>,
              B: Default + Send + ParallelExtend<R>,
              P: Fn(Self::Item) -> Either<L, R> + Sync + Send,
              L: Send,
              R: Send
    {
        unzip::partition_map(self, predicate)
    }

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    ///
    /// This method causes the iterator `self` to start producing
    /// items and to feed them to the consumer `consumer` one by one.
    /// It may split the consumer before doing so to create the
    /// opportunity to produce in parallel.
    ///
    /// See the [README] for more details on the internals of parallel
    /// iterators.
    ///
    /// [README]: README.md
    fn drive_unindexed<C>(self, consumer: C) -> C::Result where C: UnindexedConsumer<Self::Item>;


    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    ///
    /// Returns the number of items produced by this iterator, if known
    /// statically. This can be used by consumers to trigger special fast
    /// paths. Therefore, if `Some(_)` is returned, this iterator must only
    /// use the (indexed) `Consumer` methods when driving a consumer, such
    /// as `split_at()`. Calling `UnindexedConsumer::split_off_left()` or
    /// other `UnindexedConsumer` methods -- or returning an inaccurate
    /// value -- may result in panics.
    ///
    /// This method is currently used to optimize `collect` for want
    /// of true Rust specialization; it may be removed when
    /// specialization is stable.
    fn opt_len(&mut self) -> Option<usize> {
        None
    }
}

impl<T: ParallelIterator> IntoParallelIterator for T {
    type Iter = T;
    type Item = T::Item;

    fn into_par_iter(self) -> T {
        self
    }
}

/// An iterator that supports "random access" to its data, meaning
/// that you can split it at arbitrary indices and draw data from
/// those points.
pub trait IndexedParallelIterator: ParallelIterator {
    /// Collects the results of the iterator into the specified
    /// vector. The vector is always truncated before execution
    /// begins. If possible, reusing the vector across calls can lead
    /// to better performance since it reuses the same backing buffer.
    fn collect_into(self, target: &mut Vec<Self::Item>) {
        collect::collect_into(self, target);
    }

    /// Unzips the results of the iterator into the specified
    /// vectors. The vectors are always truncated before execution
    /// begins. If possible, reusing the vectors across calls can lead
    /// to better performance since they reuse the same backing buffer.
    fn unzip_into<A, B>(self, left: &mut Vec<A>, right: &mut Vec<B>)
        where Self: IndexedParallelIterator<Item = (A, B)>,
              A: Send,
              B: Send
    {
        collect::unzip_into(self, left, right);
    }

    /// Iterate over tuples `(A, B)`, where the items `A` are from
    /// this iterator and `B` are from the iterator given as argument.
    /// Like the `zip` method on ordinary iterators, if the two
    /// iterators are of unequal length, you only get the items they
    /// have in common.
    fn zip<Z>(self, zip_op: Z) -> Zip<Self, Z::Iter>
        where Z: IntoParallelIterator,
              Z::Iter: IndexedParallelIterator
    {
        zip::new(self, zip_op.into_par_iter())
    }

    /// Lexicographically compares the elements of this `ParallelIterator` with those of
    /// another.
    fn cmp<I>(mut self, other: I) -> Ordering
        where I: IntoParallelIterator<Item = Self::Item>,
              I::Iter: IndexedParallelIterator,
              Self::Item: Ord
    {
        let mut other = other.into_par_iter();
        let ord_len = self.len().cmp(&other.len());
        self.zip(other)
            .map(|(x, y)| Ord::cmp(&x, &y))
            .find_first(|&ord| ord != Ordering::Equal)
            .unwrap_or(ord_len)
    }

    /// Lexicographically compares the elements of this `ParallelIterator` with those of
    /// another.
    fn partial_cmp<I>(mut self, other: I) -> Option<Ordering>
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialOrd<I::Item>
    {
        let mut other = other.into_par_iter();
        let ord_len = self.len().cmp(&other.len());
        self.zip(other)
            .map(|(x, y)| PartialOrd::partial_cmp(&x, &y))
            .find_first(|&ord| ord != Some(Ordering::Equal))
            .unwrap_or(Some(ord_len))
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are equal to those of another
    fn eq<I>(mut self, other: I) -> bool
        where I: IntoParallelIterator,
              I::Iter: IndexedParallelIterator,
              Self::Item: PartialEq<I::Item>
    {
        let mut other = other.into_par_iter();
        self.len() == other.len() && self.zip(other).all(|(x, y)| x.eq(&y))
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
        enumerate::new(self)
    }

    /// Creates an iterator that skips the first `n` elements.
    fn skip(self, n: usize) -> Skip<Self> {
        skip::new(self, n)
    }

    /// Creates an iterator that yields the first `n` elements.
    fn take(self, n: usize) -> Take<Self> {
        take::new(self, n)
    }

    /// Searches for **some** item in the parallel iterator that
    /// matches the given predicate, and returns its index.  Like
    /// `ParallelIterator::find_any`, the parallel search will not
    /// necessarily find the **first** match, and once a match is
    /// found we'll attempt to stop processing any more.
    fn position_any<P>(self, predicate: P) -> Option<usize>
        where P: Fn(Self::Item) -> bool + Sync + Send
    {
        self.map(predicate)
            .enumerate()
            .find_any(|&(_, p)| p)
            .map(|(i, _)| i)
    }

    /// Searches for the sequentially **first** item in the parallel iterator
    /// that matches the given predicate, and returns its index.
    ///
    /// Like `ParallelIterator::find_first`, once a match is found,
    /// all attempts to the right of the match will be stopped, while
    /// attempts to the left must continue in case an earlier match
    /// is found.
    ///
    /// Note that not all parallel iterators have a useful order, much like
    /// sequential `HashMap` iteration, so "first" may be nebulous.  If you
    /// just want the first match that discovered anywhere in the iterator,
    /// `position_any` is a better choice.
    fn position_first<P>(self, predicate: P) -> Option<usize>
        where P: Fn(Self::Item) -> bool + Sync + Send
    {
        self.map(predicate)
            .enumerate()
            .find_first(|&(_, p)| p)
            .map(|(i, _)| i)
    }

    /// Searches for the sequentially **last** item in the parallel iterator
    /// that matches the given predicate, and returns its index.
    ///
    /// Like `ParallelIterator::find_last`, once a match is found,
    /// all attempts to the left of the match will be stopped, while
    /// attempts to the right must continue in case a later match
    /// is found.
    ///
    /// Note that not all parallel iterators have a useful order, much like
    /// sequential `HashMap` iteration, so "last" may be nebulous.  When the
    /// order doesn't actually matter to you, `position_any` is a better
    /// choice.
    fn position_last<P>(self, predicate: P) -> Option<usize>
        where P: Fn(Self::Item) -> bool + Sync + Send
    {
        self.map(predicate)
            .enumerate()
            .find_last(|&(_, p)| p)
            .map(|(i, _)| i)
    }

    #[doc(hidden)]
    #[deprecated(note = "parallel `position` does not search in order -- use `position_any`, \\
    `position_first`, or `position_last`")]
    fn position<P>(self, predicate: P) -> Option<usize>
        where P: Fn(Self::Item) -> bool + Sync + Send
    {
        self.position_any(predicate)
    }

    /// Produces a new iterator with the elements of this iterator in
    /// reverse order.
    fn rev(self) -> Rev<Self> {
        rev::new(self)
    }

    /// Sets the minimum length of iterators desired to process in each
    /// thread.  Rayon will not split any smaller than this length, but
    /// of course an iterator could already be smaller to begin with.
    fn with_min_len(self, min: usize) -> MinLen<Self> {
        len::new_min_len(self, min)
    }

    /// Sets the maximum length of iterators desired to process in each
    /// thread.  Rayon will try to split at least below this length,
    /// unless that would put it below the length from `with_min_len()`.
    /// For example, given min=10 and max=15, a length of 16 will not be
    /// split any further.
    fn with_max_len(self, max: usize) -> MaxLen<Self> {
        len::new_max_len(self, max)
    }

    /// Produces an exact count of how many items this iterator will
    /// produce, presuming no panic occurs.
    fn len(&mut self) -> usize;

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    ///
    /// This method causes the iterator `self` to start producing
    /// items and to feed them to the consumer `consumer` one by one.
    /// It may split the consumer before doing so to create the
    /// opportunity to produce in parallel. If a split does happen, it
    /// will inform the consumer of the index where the split should
    /// occur (unlike `ParallelIterator::drive_unindexed()`).
    ///
    /// See the [README] for more details on the internals of parallel
    /// iterators.
    ///
    /// [README]: README.md
    fn drive<'c, C: Consumer<Self::Item>>(self, consumer: C) -> C::Result;

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    ///
    /// This method converts the iterator into a producer P and then
    /// invokes `callback.callback()` with P. Note that the type of
    /// this producer is not defined as part of the API, since
    /// `callback` must be defined generically for all producers. This
    /// allows the producer type to contain references; it also means
    /// that parallel iterators can adjust that type without causing a
    /// breaking change.
    ///
    /// See the [README] for more details on the internals of parallel
    /// iterators.
    ///
    /// [README]: README.md
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output;
}

/// `FromParallelIterator` implements the conversion from a [`ParallelIterator`].
/// By implementing `FromParallelIterator` for a type, you define how it will be
/// created from an iterator.
///
/// `FromParallelIterator` is used through [`ParallelIterator`]'s [`collect()`] method.
///
/// [`ParallelIterator`]: trait.ParallelIterator.html
/// [`collect()`]: trait.ParallelIterator.html#method.collect
pub trait FromParallelIterator<T>
    where T: Send
{
    fn from_par_iter<I>(par_iter: I) -> Self where I: IntoParallelIterator<Item = T>;
}

/// `ParallelExtend` extends an existing collection with items from a [`ParallelIterator`].
///
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub trait ParallelExtend<T>
    where T: Send
{
    fn par_extend<I>(&mut self, par_iter: I) where I: IntoParallelIterator<Item = T>;
}
