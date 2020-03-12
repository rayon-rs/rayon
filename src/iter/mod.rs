//! Traits for writing parallel programs using an iterator-style interface
//!
//! You will rarely need to interact with this module directly unless you have
//! need to name one of the iterator types.
//!
//! Parallel iterators make it easy to write iterator-like chains that
//! execute in parallel: typically all you have to do is convert the
//! first `.iter()` (or `iter_mut()`, `into_iter()`, etc) method into
//! `par_iter()` (or `par_iter_mut()`, `into_par_iter()`, etc). For
//! example, to compute the sum of the squares of a sequence of
//! integers, one might write:
//!
//! ```rust
//! use rayon::prelude::*;
//! fn sum_of_squares(input: &[i32]) -> i32 {
//!     input.par_iter()
//!          .map(|i| i * i)
//!          .sum()
//! }
//! ```
//!
//! Or, to increment all the integers in a slice, you could write:
//!
//! ```rust
//! use rayon::prelude::*;
//! fn increment_all(input: &mut [i32]) {
//!     input.par_iter_mut()
//!          .for_each(|p| *p += 1);
//! }
//! ```
//!
//! To use parallel iterators, first import the traits by adding
//! something like `use rayon::prelude::*` to your module. You can
//! then call `par_iter`, `par_iter_mut`, or `into_par_iter` to get a
//! parallel iterator. Like a [regular iterator][], parallel
//! iterators work by first constructing a computation and then
//! executing it.
//!
//! In addition to `par_iter()` and friends, some types offer other
//! ways to create (or consume) parallel iterators:
//!
//! - Slices (`&[T]`, `&mut [T]`) offer methods like `par_split` and
//!   `par_windows`, as well as various parallel sorting
//!   operations. See [the `ParallelSlice` trait] for the full list.
//! - Strings (`&str`) offer methods like `par_split` and `par_lines`.
//!   See [the `ParallelString` trait] for the full list.
//! - Various collections offer [`par_extend`], which grows a
//!   collection given a parallel iterator. (If you don't have a
//!   collection to extend, you can use [`collect()`] to create a new
//!   one from scratch.)
//!
//! [the `ParallelSlice` trait]: ../slice/trait.ParallelSlice.html
//! [the `ParallelString` trait]: ../str/trait.ParallelString.html
//! [`par_extend`]: trait.ParallelExtend.html
//! [`collect()`]: trait.ParallelIterator.html#method.collect
//!
//! To see the full range of methods available on parallel iterators,
//! check out the [`ParallelIterator`] and [`IndexedParallelIterator`]
//! traits.
//!
//! If you'd like to build a custom parallel iterator, or to write your own
//! combinator, then check out the [split] function and the [plumbing] module.
//!
//! [regular iterator]: http://doc.rust-lang.org/std/iter/trait.Iterator.html
//! [`ParallelIterator`]: trait.ParallelIterator.html
//! [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
//! [split]: fn.split.html
//! [plumbing]: plumbing/index.html
//!
//! Note: Several of the `ParallelIterator` methods rely on a `Try` trait which
//! has been deliberately obscured from the public API.  This trait is intended
//! to mirror the unstable `std::ops::Try` with implementations for `Option` and
//! `Result`, where `Some`/`Ok` values will let those iterators continue, but
//! `None`/`Err` values will exit early.
//!
//! A note about object safety: It is currently _not_ possible to wrap
//! a `ParallelIterator` (or any trait that depends on it) using a
//! `Box<dyn ParallelIterator>` or other kind of dynamic allocation,
//! because `ParallelIterator` is **not object-safe**.
//! (This keeps the implementation simpler and allows extra optimizations.)

use self::plumbing::*;
use self::private::Try;
pub use either::Either;
use std::cmp::{self, Ordering};
use std::iter::{Product, Sum};
use std::ops::Fn;

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

mod par_bridge;
pub use self::par_bridge::{IterBridge, ParallelBridge};

pub mod scheduler;
pub use self::scheduler::{Scheduler, UnindexedScheduler, WithScheduler, WithUnindexedScheduler};
pub use self::scheduler::misc::*;

mod chain;
mod find;
mod find_first_last;
pub use self::chain::Chain;
mod chunks;
pub use self::chunks::Chunks;
mod collect;
mod enumerate;
pub use self::enumerate::Enumerate;
mod filter;
pub use self::filter::Filter;
mod filter_map;
pub use self::filter_map::FilterMap;
mod flat_map;
pub use self::flat_map::FlatMap;
mod flatten;
pub use self::flatten::Flatten;
mod fold;
mod for_each;
mod from_par_iter;
pub mod plumbing;
pub use self::fold::{Fold, FoldWith};
mod try_fold;
pub use self::try_fold::{TryFold, TryFoldWith};
mod reduce;
mod skip;
mod try_reduce;
mod try_reduce_with;
pub use self::skip::Skip;
mod splitter;
pub use self::splitter::{split, Split};
mod take;
pub use self::take::Take;
mod map;
pub use self::map::Map;
mod map_with;
pub use self::map_with::{MapInit, MapWith};
mod zip;
pub use self::zip::Zip;
mod zip_eq;
pub use self::zip_eq::ZipEq;
mod multizip;
pub use self::multizip::MultiZip;
mod interleave;
pub use self::interleave::Interleave;
mod interleave_shortest;
pub use self::interleave_shortest::InterleaveShortest;
mod intersperse;
pub use self::intersperse::Intersperse;
mod update;
pub use self::update::Update;
mod step_by;
#[cfg(step_by)]
pub use self::step_by::StepBy;

mod noop;
mod rev;
pub use self::rev::Rev;
mod len;
pub use self::len::{MaxLen, MinLen};

mod cloned;
pub use self::cloned::Cloned;
mod copied;
pub use self::copied::Copied;

mod product;
mod sum;

mod inspect;
pub use self::inspect::Inspect;
mod panic_fuse;
pub use self::panic_fuse::PanicFuse;
mod while_some;
pub use self::while_some::WhileSome;
mod extend;
mod repeat;
mod unzip;
pub use self::repeat::{repeat, Repeat};
pub use self::repeat::{repeatn, RepeatN};

mod empty;
pub use self::empty::{empty, Empty};
mod once;
pub use self::once::{once, Once};

#[cfg(test)]
mod test;

/// `IntoParallelIterator` implements the conversion to a [`ParallelIterator`].
///
/// By implementing `IntoParallelIterator` for a type, you define how it will
/// transformed into an iterator. This is a parallel version of the standard
/// library's [`std::iter::IntoIterator`] trait.
///
/// [`ParallelIterator`]: trait.ParallelIterator.html
/// [`std::iter::IntoIterator`]: https://doc.rust-lang.org/std/iter/trait.IntoIterator.html
pub trait IntoParallelIterator {
    /// The parallel iterator type that will be created.
    type Iter: ParallelIterator<Item = Self::Item>;

    /// The type of item that the parallel iterator will produce.
    type Item: Send;

    /// Converts `self` into a parallel iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// println!("counting in parallel:");
    /// (0..100).into_par_iter()
    ///     .for_each(|i| println!("{}", i));
    /// ```
    ///
    /// This conversion is often implicit for arguments to methods like [`zip`].
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let v: Vec<_> = (0..5).into_par_iter().zip(5..10).collect();
    /// assert_eq!(v, [(0, 5), (1, 6), (2, 7), (3, 8), (4, 9)]);
    /// ```
    ///
    /// [`zip`]: trait.IndexedParallelIterator.html#method.zip
    fn into_par_iter(self) -> Self::Iter;
}

/// `IntoParallelRefIterator` implements the conversion to a
/// [`ParallelIterator`], providing shared references to the data.
///
/// This is a parallel version of the `iter()` method
/// defined by various collections.
///
/// This trait is automatically implemented
/// `for I where &I: IntoParallelIterator`. In most cases, users
/// will want to implement [`IntoParallelIterator`] rather than implement
/// this trait directly.
///
/// [`ParallelIterator`]: trait.ParallelIterator.html
/// [`IntoParallelIterator`]: trait.IntoParallelIterator.html
pub trait IntoParallelRefIterator<'data> {
    /// The type of the parallel iterator that will be returned.
    type Iter: ParallelIterator<Item = Self::Item>;

    /// The type of item that the parallel iterator will produce.
    /// This will typically be an `&'data T` reference type.
    type Item: Send + 'data;

    /// Converts `self` into a parallel iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let v: Vec<_> = (0..100).collect();
    /// assert_eq!(v.par_iter().sum::<i32>(), 100 * 99 / 2);
    ///
    /// // `v.par_iter()` is shorthand for `(&v).into_par_iter()`,
    /// // producing the exact same references.
    /// assert!(v.par_iter().zip(&v)
    ///          .all(|(a, b)| std::ptr::eq(a, b)));
    /// ```
    fn par_iter(&'data self) -> Self::Iter;
}

impl<'data, I: 'data + ?Sized> IntoParallelRefIterator<'data> for I
where
    &'data I: IntoParallelIterator,
{
    type Iter = <&'data I as IntoParallelIterator>::Iter;
    type Item = <&'data I as IntoParallelIterator>::Item;

    fn par_iter(&'data self) -> Self::Iter {
        self.into_par_iter()
    }
}

/// `IntoParallelRefMutIterator` implements the conversion to a
/// [`ParallelIterator`], providing mutable references to the data.
///
/// This is a parallel version of the `iter_mut()` method
/// defined by various collections.
///
/// This trait is automatically implemented
/// `for I where &mut I: IntoParallelIterator`. In most cases, users
/// will want to implement [`IntoParallelIterator`] rather than implement
/// this trait directly.
///
/// [`ParallelIterator`]: trait.ParallelIterator.html
/// [`IntoParallelIterator`]: trait.IntoParallelIterator.html
pub trait IntoParallelRefMutIterator<'data> {
    /// The type of iterator that will be created.
    type Iter: ParallelIterator<Item = Self::Item>;

    /// The type of item that will be produced; this is typically an
    /// `&'data mut T` reference.
    type Item: Send + 'data;

    /// Creates the parallel iterator from `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let mut v = vec![0usize; 5];
    /// v.par_iter_mut().enumerate().for_each(|(i, x)| *x = i);
    /// assert_eq!(v, [0, 1, 2, 3, 4]);
    /// ```
    fn par_iter_mut(&'data mut self) -> Self::Iter;
}

impl<'data, I: 'data + ?Sized> IntoParallelRefMutIterator<'data> for I
where
    &'data mut I: IntoParallelIterator,
{
    type Iter = <&'data mut I as IntoParallelIterator>::Iter;
    type Item = <&'data mut I as IntoParallelIterator>::Item;

    fn par_iter_mut(&'data mut self) -> Self::Iter {
        self.into_par_iter()
    }
}

/// Parallel version of the standard iterator trait.
///
/// The combinators on this trait are available on **all** parallel
/// iterators.  Additional methods can be found on the
/// [`IndexedParallelIterator`] trait: those methods are only
/// available for parallel iterators where the number of items is
/// known in advance (so, e.g., after invoking `filter`, those methods
/// become unavailable).
///
/// For examples of using parallel iterators, see [the docs on the
/// `iter` module][iter].
///
/// [iter]: index.html
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
pub trait ParallelIterator: Sized + Send {
    /// The type of item that this parallel iterator produces.
    /// For example, if you use the [`for_each`] method, this is the type of
    /// item that your closure will be invoked with.
    ///
    /// [`for_each`]: #method.for_each
    type Item: Send;

    /// Sets the "scheduler" that this thread-pool will use.
    fn with_unindexed_scheduler<S>(self, scheduler: S) -> WithUnindexedScheduler<Self, S> {
        WithUnindexedScheduler::new(self, scheduler)
    }

    /// Executes `OP` on each item produced by the iterator, in parallel.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// (0..100).into_par_iter().for_each(|x| println!("{:?}", x));
    /// ```
    fn for_each<OP>(self, op: OP)
    where
        OP: Fn(Self::Item) + Sync + Send,
    {
        for_each::for_each(self, &op)
    }

    /// Executes `OP` on the given `init` value with each item produced by
    /// the iterator, in parallel.
    ///
    /// The `init` value will be cloned only as needed to be paired with
    /// the group of items in each rayon job.  It does not require the type
    /// to be `Sync`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::mpsc::channel;
    /// use rayon::prelude::*;
    ///
    /// let (sender, receiver) = channel();
    ///
    /// (0..5).into_par_iter().for_each_with(sender, |s, x| s.send(x).unwrap());
    ///
    /// let mut res: Vec<_> = receiver.iter().collect();
    ///
    /// res.sort();
    ///
    /// assert_eq!(&res[..], &[0, 1, 2, 3, 4])
    /// ```
    fn for_each_with<OP, T>(self, init: T, op: OP)
    where
        OP: Fn(&mut T, Self::Item) + Sync + Send,
        T: Send + Clone,
    {
        self.map_with(init, op).collect()
    }

    /// Executes `OP` on a value returned by `init` with each item produced by
    /// the iterator, in parallel.
    ///
    /// The `init` function will be called only as needed for a value to be
    /// paired with the group of items in each rayon job.  There is no
    /// constraint on that returned type at all!
    ///
    /// # Examples
    ///
    /// ```
    /// use rand::Rng;
    /// use rayon::prelude::*;
    ///
    /// let mut v = vec![0u8; 1_000_000];
    ///
    /// v.par_chunks_mut(1000)
    ///     .for_each_init(
    ///         || rand::thread_rng(),
    ///         |rng, chunk| rng.fill(chunk),
    ///     );
    ///
    /// // There's a remote chance that this will fail...
    /// for i in 0u8..=255 {
    ///     assert!(v.contains(&i));
    /// }
    /// ```
    fn for_each_init<OP, INIT, T>(self, init: INIT, op: OP)
    where
        OP: Fn(&mut T, Self::Item) + Sync + Send,
        INIT: Fn() -> T + Sync + Send,
    {
        self.map_init(init, op).collect()
    }

    /// Executes a fallible `OP` on each item produced by the iterator, in parallel.
    ///
    /// If the `OP` returns `Result::Err` or `Option::None`, we will attempt to
    /// stop processing the rest of the items in the iterator as soon as
    /// possible, and we will return that terminating value.  Otherwise, we will
    /// return an empty `Result::Ok(())` or `Option::Some(())`.  If there are
    /// multiple errors in parallel, it is not specified which will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// use std::io::{self, Write};
    ///
    /// // This will stop iteration early if there's any write error, like
    /// // having piped output get closed on the other end.
    /// (0..100).into_par_iter()
    ///     .try_for_each(|x| writeln!(io::stdout(), "{:?}", x))
    ///     .expect("expected no write errors");
    /// ```
    fn try_for_each<OP, R>(self, op: OP) -> R
    where
        OP: Fn(Self::Item) -> R + Sync + Send,
        R: Try<Ok = ()> + Send,
    {
        fn ok<R: Try<Ok = ()>>(_: (), _: ()) -> R {
            R::from_ok(())
        }

        self.map(op).try_reduce(<()>::default, ok)
    }

    /// Executes a fallible `OP` on the given `init` value with each item
    /// produced by the iterator, in parallel.
    ///
    /// This combines the `init` semantics of [`for_each_with()`] and the
    /// failure semantics of [`try_for_each()`].
    ///
    /// [`for_each_with()`]: #method.for_each_with
    /// [`try_for_each()`]: #method.try_for_each
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::mpsc::channel;
    /// use rayon::prelude::*;
    ///
    /// let (sender, receiver) = channel();
    ///
    /// (0..5).into_par_iter()
    ///     .try_for_each_with(sender, |s, x| s.send(x))
    ///     .expect("expected no send errors");
    ///
    /// let mut res: Vec<_> = receiver.iter().collect();
    ///
    /// res.sort();
    ///
    /// assert_eq!(&res[..], &[0, 1, 2, 3, 4])
    /// ```
    fn try_for_each_with<OP, T, R>(self, init: T, op: OP) -> R
    where
        OP: Fn(&mut T, Self::Item) -> R + Sync + Send,
        T: Send + Clone,
        R: Try<Ok = ()> + Send,
    {
        fn ok<R: Try<Ok = ()>>(_: (), _: ()) -> R {
            R::from_ok(())
        }

        self.map_with(init, op).try_reduce(<()>::default, ok)
    }

    /// Executes a fallible `OP` on a value returned by `init` with each item
    /// produced by the iterator, in parallel.
    ///
    /// This combines the `init` semantics of [`for_each_init()`] and the
    /// failure semantics of [`try_for_each()`].
    ///
    /// [`for_each_init()`]: #method.for_each_init
    /// [`try_for_each()`]: #method.try_for_each
    ///
    /// # Examples
    ///
    /// ```
    /// use rand::Rng;
    /// use rayon::prelude::*;
    ///
    /// let mut v = vec![0u8; 1_000_000];
    ///
    /// v.par_chunks_mut(1000)
    ///     .try_for_each_init(
    ///         || rand::thread_rng(),
    ///         |rng, chunk| rng.try_fill(chunk),
    ///     )
    ///     .expect("expected no rand errors");
    ///
    /// // There's a remote chance that this will fail...
    /// for i in 0u8..=255 {
    ///     assert!(v.contains(&i));
    /// }
    /// ```
    fn try_for_each_init<OP, INIT, T, R>(self, init: INIT, op: OP) -> R
    where
        OP: Fn(&mut T, Self::Item) -> R + Sync + Send,
        INIT: Fn() -> T + Sync + Send,
        R: Try<Ok = ()> + Send,
    {
        fn ok<R: Try<Ok = ()>>(_: (), _: ()) -> R {
            R::from_ok(())
        }

        self.map_init(init, op).try_reduce(<()>::default, ok)
    }

    /// Counts the number of items in this parallel iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let count = (0..100).into_par_iter().count();
    ///
    /// assert_eq!(count, 100);
    /// ```
    fn count(self) -> usize {
        fn one<T>(_: T) -> usize {
            1
        }

        self.map(one).sum()
    }

    /// Applies `map_op` to each item of this iterator, producing a new
    /// iterator with the results.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let mut par_iter = (0..5).into_par_iter().map(|x| x * 2);
    ///
    /// let doubles: Vec<_> = par_iter.collect();
    ///
    /// assert_eq!(&doubles[..], &[0, 2, 4, 6, 8]);
    /// ```
    fn map<F, R>(self, map_op: F) -> Map<Self, F>
    where
        F: Fn(Self::Item) -> R + Sync + Send,
        R: Send,
    {
        Map::new(self, map_op)
    }

    /// Applies `map_op` to the given `init` value with each item of this
    /// iterator, producing a new iterator with the results.
    ///
    /// The `init` value will be cloned only as needed to be paired with
    /// the group of items in each rayon job.  It does not require the type
    /// to be `Sync`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::mpsc::channel;
    /// use rayon::prelude::*;
    ///
    /// let (sender, receiver) = channel();
    ///
    /// let a: Vec<_> = (0..5)
    ///                 .into_par_iter()            // iterating over i32
    ///                 .map_with(sender, |s, x| {
    ///                     s.send(x).unwrap();     // sending i32 values through the channel
    ///                     x                       // returning i32
    ///                 })
    ///                 .collect();                 // collecting the returned values into a vector
    ///
    /// let mut b: Vec<_> = receiver.iter()         // iterating over the values in the channel
    ///                             .collect();     // and collecting them
    /// b.sort();
    ///
    /// assert_eq!(a, b);
    /// ```
    fn map_with<F, T, R>(self, init: T, map_op: F) -> MapWith<Self, T, F>
    where
        F: Fn(&mut T, Self::Item) -> R + Sync + Send,
        T: Send + Clone,
        R: Send,
    {
        MapWith::new(self, init, map_op)
    }

    /// Applies `map_op` to a value returned by `init` with each item of this
    /// iterator, producing a new iterator with the results.
    ///
    /// The `init` function will be called only as needed for a value to be
    /// paired with the group of items in each rayon job.  There is no
    /// constraint on that returned type at all!
    ///
    /// # Examples
    ///
    /// ```
    /// use rand::Rng;
    /// use rayon::prelude::*;
    ///
    /// let a: Vec<_> = (1i32..1_000_000)
    ///     .into_par_iter()
    ///     .map_init(
    ///         || rand::thread_rng(),  // get the thread-local RNG
    ///         |rng, x| if rng.gen() { // randomly negate items
    ///             -x
    ///         } else {
    ///             x
    ///         },
    ///     ).collect();
    ///
    /// // There's a remote chance that this will fail...
    /// assert!(a.iter().any(|&x| x < 0));
    /// assert!(a.iter().any(|&x| x > 0));
    /// ```
    fn map_init<F, INIT, T, R>(self, init: INIT, map_op: F) -> MapInit<Self, INIT, F>
    where
        F: Fn(&mut T, Self::Item) -> R + Sync + Send,
        INIT: Fn() -> T + Sync + Send,
        R: Send,
    {
        MapInit::new(self, init, map_op)
    }

    /// Creates an iterator which clones all of its elements.  This may be
    /// useful when you have an iterator over `&T`, but you need `T`, and
    /// that type implements `Clone`. See also [`copied()`].
    ///
    /// [`copied()`]: #method.copied
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 2, 3];
    ///
    /// let v_cloned: Vec<_> = a.par_iter().cloned().collect();
    ///
    /// // cloned is the same as .map(|&x| x), for integers
    /// let v_map: Vec<_> = a.par_iter().map(|&x| x).collect();
    ///
    /// assert_eq!(v_cloned, vec![1, 2, 3]);
    /// assert_eq!(v_map, vec![1, 2, 3]);
    /// ```
    fn cloned<'a, T>(self) -> Cloned<Self>
    where
        T: 'a + Clone + Send,
        Self: ParallelIterator<Item = &'a T>,
    {
        Cloned::new(self)
    }

    /// Creates an iterator which copies all of its elements.  This may be
    /// useful when you have an iterator over `&T`, but you need `T`, and
    /// that type implements `Copy`. See also [`cloned()`].
    ///
    /// [`cloned()`]: #method.cloned
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 2, 3];
    ///
    /// let v_copied: Vec<_> = a.par_iter().copied().collect();
    ///
    /// // copied is the same as .map(|&x| x), for integers
    /// let v_map: Vec<_> = a.par_iter().map(|&x| x).collect();
    ///
    /// assert_eq!(v_copied, vec![1, 2, 3]);
    /// assert_eq!(v_map, vec![1, 2, 3]);
    /// ```
    fn copied<'a, T>(self) -> Copied<Self>
    where
        T: 'a + Copy + Send,
        Self: ParallelIterator<Item = &'a T>,
    {
        Copied::new(self)
    }

    /// Applies `inspect_op` to a reference to each item of this iterator,
    /// producing a new iterator passing through the original items.  This is
    /// often useful for debugging to see what's happening in iterator stages.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 4, 2, 3];
    ///
    /// // this iterator sequence is complex.
    /// let sum = a.par_iter()
    ///             .cloned()
    ///             .filter(|&x| x % 2 == 0)
    ///             .reduce(|| 0, |sum, i| sum + i);
    ///
    /// println!("{}", sum);
    ///
    /// // let's add some inspect() calls to investigate what's happening
    /// let sum = a.par_iter()
    ///             .cloned()
    ///             .inspect(|x| println!("about to filter: {}", x))
    ///             .filter(|&x| x % 2 == 0)
    ///             .inspect(|x| println!("made it through filter: {}", x))
    ///             .reduce(|| 0, |sum, i| sum + i);
    ///
    /// println!("{}", sum);
    /// ```
    fn inspect<OP>(self, inspect_op: OP) -> Inspect<Self, OP>
    where
        OP: Fn(&Self::Item) + Sync + Send,
    {
        Inspect::new(self, inspect_op)
    }

    /// Mutates each item of this iterator before yielding it.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let par_iter = (0..5).into_par_iter().update(|x| {*x *= 2;});
    ///
    /// let doubles: Vec<_> = par_iter.collect();
    ///
    /// assert_eq!(&doubles[..], &[0, 2, 4, 6, 8]);
    /// ```
    fn update<F>(self, update_op: F) -> Update<Self, F>
    where
        F: Fn(&mut Self::Item) + Sync + Send,
    {
        Update::new(self, update_op)
    }

    /// Applies `filter_op` to each item of this iterator, producing a new
    /// iterator with only the items that gave `true` results.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let mut par_iter = (0..10).into_par_iter().filter(|x| x % 2 == 0);
    ///
    /// let even_numbers: Vec<_> = par_iter.collect();
    ///
    /// assert_eq!(&even_numbers[..], &[0, 2, 4, 6, 8]);
    /// ```
    fn filter<P>(self, filter_op: P) -> Filter<Self, P>
    where
        P: Fn(&Self::Item) -> bool + Sync + Send,
    {
        Filter::new(self, filter_op)
    }

    /// Applies `filter_op` to each item of this iterator to get an `Option`,
    /// producing a new iterator with only the items from `Some` results.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let mut par_iter = (0..10).into_par_iter()
    ///                         .filter_map(|x| {
    ///                             if x % 2 == 0 { Some(x * 3) }
    ///                             else { None }
    ///                         });
    ///
    /// let even_numbers: Vec<_> = par_iter.collect();
    ///
    /// assert_eq!(&even_numbers[..], &[0, 6, 12, 18, 24]);
    /// ```
    fn filter_map<P, R>(self, filter_op: P) -> FilterMap<Self, P>
    where
        P: Fn(Self::Item) -> Option<R> + Sync + Send,
        R: Send,
    {
        FilterMap::new(self, filter_op)
    }

    /// Applies `map_op` to each item of this iterator to get nested iterators,
    /// producing a new iterator that flattens these back into one.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [[1, 2], [3, 4], [5, 6], [7, 8]];
    ///
    /// let par_iter = a.par_iter().cloned().flat_map(|a| a.to_vec());
    ///
    /// let vec: Vec<_> = par_iter.collect();
    ///
    /// assert_eq!(&vec[..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    /// ```
    fn flat_map<F, PI>(self, map_op: F) -> FlatMap<Self, F>
    where
        F: Fn(Self::Item) -> PI + Sync + Send,
        PI: IntoParallelIterator,
    {
        FlatMap::new(self, map_op)
    }

    /// An adaptor that flattens iterable `Item`s into one large iterator
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let x: Vec<Vec<_>> = vec![vec![1, 2], vec![3, 4]];
    /// let y: Vec<_> = x.into_par_iter().flatten().collect();
    ///
    /// assert_eq!(y, vec![1, 2, 3, 4]);
    /// ```
    fn flatten(self) -> Flatten<Self>
    where
        Self::Item: IntoParallelIterator,
    {
        Flatten::new(self)
    }

    /// Reduces the items in the iterator into one item using `op`.
    /// The argument `identity` should be a closure that can produce
    /// "identity" value which may be inserted into the sequence as
    /// needed to create opportunities for parallel execution. So, for
    /// example, if you are doing a summation, then `identity()` ought
    /// to produce something that represents the zero for your type
    /// (but consider just calling `sum()` in that case).
    ///
    /// # Examples
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
    where
        OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync + Send,
        ID: Fn() -> Self::Item + Sync + Send,
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
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// let sums = [(0, 1), (5, 6), (16, 2), (8, 9)]
    ///            .par_iter()        // iterating over &(i32, i32)
    ///            .cloned()          // iterating over (i32, i32)
    ///            .reduce_with(|a, b| (a.0 + b.0, a.1 + b.1))
    ///            .unwrap();
    /// assert_eq!(sums, (0 + 5 + 16 + 8, 1 + 6 + 2 + 9));
    /// ```
    ///
    /// **Note:** unlike a sequential `fold` operation, the order in
    /// which `op` will be applied to reduce the result is not fully
    /// specified. So `op` should be [associative] or else the results
    /// will be non-deterministic.
    ///
    /// [associative]: https://en.wikipedia.org/wiki/Associative_property
    fn reduce_with<OP>(self, op: OP) -> Option<Self::Item>
    where
        OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync + Send,
    {
        fn opt_fold<T>(op: impl Fn(T, T) -> T) -> impl Fn(Option<T>, T) -> Option<T> {
            move |opt_a, b| match opt_a {
                Some(a) => Some(op(a, b)),
                None => Some(b),
            }
        }

        fn opt_reduce<T>(op: impl Fn(T, T) -> T) -> impl Fn(Option<T>, Option<T>) -> Option<T> {
            move |opt_a, opt_b| match (opt_a, opt_b) {
                (Some(a), Some(b)) => Some(op(a, b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            }
        }

        self.fold(<_>::default, opt_fold(&op))
            .reduce(<_>::default, opt_reduce(&op))
    }

    /// Reduces the items in the iterator into one item using a fallible `op`.
    /// The `identity` argument is used the same way as in [`reduce()`].
    ///
    /// [`reduce()`]: #method.reduce
    ///
    /// If a `Result::Err` or `Option::None` item is found, or if `op` reduces
    /// to one, we will attempt to stop processing the rest of the items in the
    /// iterator as soon as possible, and we will return that terminating value.
    /// Otherwise, we will return the final reduced `Result::Ok(T)` or
    /// `Option::Some(T)`.  If there are multiple errors in parallel, it is not
    /// specified which will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// // Compute the sum of squares, being careful about overflow.
    /// fn sum_squares<I: IntoParallelIterator<Item = i32>>(iter: I) -> Option<i32> {
    ///     iter.into_par_iter()
    ///         .map(|i| i.checked_mul(i))            // square each item,
    ///         .try_reduce(|| 0, i32::checked_add)   // and add them up!
    /// }
    /// assert_eq!(sum_squares(0..5), Some(0 + 1 + 4 + 9 + 16));
    ///
    /// // The sum might overflow
    /// assert_eq!(sum_squares(0..10_000), None);
    ///
    /// // Or the squares might overflow before it even reaches `try_reduce`
    /// assert_eq!(sum_squares(1_000_000..1_000_001), None);
    /// ```
    fn try_reduce<T, OP, ID>(self, identity: ID, op: OP) -> Self::Item
    where
        OP: Fn(T, T) -> Self::Item + Sync + Send,
        ID: Fn() -> T + Sync + Send,
        Self::Item: Try<Ok = T>,
    {
        try_reduce::try_reduce(self, identity, op)
    }

    /// Reduces the items in the iterator into one item using a fallible `op`.
    ///
    /// Like [`reduce_with()`], if the iterator is empty, `None` is returned;
    /// otherwise, `Some` is returned.  Beyond that, it behaves like
    /// [`try_reduce()`] for handling `Err`/`None`.
    ///
    /// [`reduce_with()`]: #method.reduce_with
    /// [`try_reduce()`]: #method.try_reduce
    ///
    /// For instance, with `Option` items, the return value may be:
    /// - `None`, the iterator was empty
    /// - `Some(None)`, we stopped after encountering `None`.
    /// - `Some(Some(x))`, the entire iterator reduced to `x`.
    ///
    /// With `Result` items, the nesting is more obvious:
    /// - `None`, the iterator was empty
    /// - `Some(Err(e))`, we stopped after encountering an error `e`.
    /// - `Some(Ok(x))`, the entire iterator reduced to `x`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let files = ["/dev/null", "/does/not/exist"];
    ///
    /// // Find the biggest file
    /// files.into_par_iter()
    ///     .map(|path| std::fs::metadata(path).map(|m| (path, m.len())))
    ///     .try_reduce_with(|a, b| {
    ///         Ok(if a.1 >= b.1 { a } else { b })
    ///     })
    ///     .expect("Some value, since the iterator is not empty")
    ///     .expect_err("not found");
    /// ```
    fn try_reduce_with<T, OP>(self, op: OP) -> Option<Self::Item>
    where
        OP: Fn(T, T) -> Self::Item + Sync + Send,
        Self::Item: Try<Ok = T>,
    {
        try_reduce_with::try_reduce_with(self, op)
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
    /// cheaper to create groups of elements at a time. For example,
    /// imagine collecting characters into a string. If you were going
    /// to use map/reduce, you might try this:
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let s =
    ///     ['a', 'b', 'c', 'd', 'e']
    ///     .par_iter()
    ///     .map(|c: &char| format!("{}", c))
    ///     .reduce(|| String::new(),
    ///             |mut a: String, b: String| { a.push_str(&b); a });
    ///
    /// assert_eq!(s, "abcde");
    /// ```
    ///
    /// Because reduce produces the same type of element as its input,
    /// you have to first map each character into a string, and then
    /// you can reduce them. This means we create one string per
    /// element in our iterator -- not so great. Using `fold`, we can
    /// do this instead:
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let s =
    ///     ['a', 'b', 'c', 'd', 'e']
    ///     .par_iter()
    ///     .fold(|| String::new(),
    ///             |mut s: String, c: &char| { s.push(*c); s })
    ///     .reduce(|| String::new(),
    ///             |mut a: String, b: String| { a.push_str(&b); a });
    ///
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
    ///
    /// let bytes = 0..22_u8;
    /// let sum = bytes.into_par_iter()
    ///                .fold(|| 0_u32, |a: u32, b: u8| a + (b as u32))
    ///                .sum::<u32>();
    ///
    /// assert_eq!(sum, (0..22).sum()); // compare to sequential
    /// ```
    fn fold<T, ID, F>(self, identity: ID, fold_op: F) -> Fold<Self, ID, F>
    where
        F: Fn(T, Self::Item) -> T + Sync + Send,
        ID: Fn() -> T + Sync + Send,
        T: Send,
    {
        Fold::new(self, identity, fold_op)
    }

    /// Applies `fold_op` to the given `init` value with each item of this
    /// iterator, finally producing the value for further use.
    ///
    /// This works essentially like `fold(|| init.clone(), fold_op)`, except
    /// it doesn't require the `init` type to be `Sync`, nor any other form
    /// of added synchronization.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let bytes = 0..22_u8;
    /// let sum = bytes.into_par_iter()
    ///                .fold_with(0_u32, |a: u32, b: u8| a + (b as u32))
    ///                .sum::<u32>();
    ///
    /// assert_eq!(sum, (0..22).sum()); // compare to sequential
    /// ```
    fn fold_with<F, T>(self, init: T, fold_op: F) -> FoldWith<Self, T, F>
    where
        F: Fn(T, Self::Item) -> T + Sync + Send,
        T: Send + Clone,
    {
        FoldWith::new(self, init, fold_op)
    }

    /// Perform a fallible parallel fold.
    ///
    /// This is a variation of [`fold()`] for operations which can fail with
    /// `Option::None` or `Result::Err`.  The first such failure stops
    /// processing the local set of items, without affecting other folds in the
    /// iterator's subdivisions.
    ///
    /// Often, `try_fold()` will be followed by [`try_reduce()`]
    /// for a final reduction and global short-circuiting effect.
    ///
    /// [`fold()`]: #method.fold
    /// [`try_reduce()`]: #method.try_reduce
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let bytes = 0..22_u8;
    /// let sum = bytes.into_par_iter()
    ///                .try_fold(|| 0_u32, |a: u32, b: u8| a.checked_add(b as u32))
    ///                .try_reduce(|| 0, u32::checked_add);
    ///
    /// assert_eq!(sum, Some((0..22).sum())); // compare to sequential
    /// ```
    fn try_fold<T, R, ID, F>(self, identity: ID, fold_op: F) -> TryFold<Self, R, ID, F>
    where
        F: Fn(T, Self::Item) -> R + Sync + Send,
        ID: Fn() -> T + Sync + Send,
        R: Try<Ok = T> + Send,
    {
        TryFold::new(self, identity, fold_op)
    }

    /// Perform a fallible parallel fold with a cloneable `init` value.
    ///
    /// This combines the `init` semantics of [`fold_with()`] and the failure
    /// semantics of [`try_fold()`].
    ///
    /// [`fold_with()`]: #method.fold_with
    /// [`try_fold()`]: #method.try_fold
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let bytes = 0..22_u8;
    /// let sum = bytes.into_par_iter()
    ///                .try_fold_with(0_u32, |a: u32, b: u8| a.checked_add(b as u32))
    ///                .try_reduce(|| 0, u32::checked_add);
    ///
    /// assert_eq!(sum, Some((0..22).sum())); // compare to sequential
    /// ```
    fn try_fold_with<F, T, R>(self, init: T, fold_op: F) -> TryFoldWith<Self, R, F>
    where
        F: Fn(T, Self::Item) -> R + Sync + Send,
        R: Try<Ok = T> + Send,
        T: Clone + Send,
    {
        TryFoldWith::new(self, init, fold_op)
    }

    /// Sums up the items in the iterator.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `+` operator is not truly [associative] \(as is the
    /// case for floating point numbers), then the results are not
    /// fully deterministic.
    ///
    /// [associative]: https://en.wikipedia.org/wiki/Associative_property
    ///
    /// Basically equivalent to `self.reduce(|| 0, |a, b| a + b)`,
    /// except that the type of `0` and the `+` operation may vary
    /// depending on the type of value being produced.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 5, 7];
    ///
    /// let sum: i32 = a.par_iter().sum();
    ///
    /// assert_eq!(sum, 13);
    /// ```
    fn sum<S>(self) -> S
    where
        S: Send + Sum<Self::Item> + Sum<S>,
    {
        sum::sum(self)
    }

    /// Multiplies all the items in the iterator.
    ///
    /// Note that the order in items will be reduced is not specified,
    /// so if the `*` operator is not truly [associative] \(as is the
    /// case for floating point numbers), then the results are not
    /// fully deterministic.
    ///
    /// [associative]: https://en.wikipedia.org/wiki/Associative_property
    ///
    /// Basically equivalent to `self.reduce(|| 1, |a, b| a * b)`,
    /// except that the type of `1` and the `*` operation may vary
    /// depending on the type of value being produced.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// fn factorial(n: u32) -> u32 {
    ///    (1..n+1).into_par_iter().product()
    /// }
    ///
    /// assert_eq!(factorial(0), 1);
    /// assert_eq!(factorial(1), 1);
    /// assert_eq!(factorial(5), 120);
    /// ```
    fn product<P>(self) -> P
    where
        P: Send + Product<Self::Item> + Product<P>,
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [45, 74, 32];
    ///
    /// assert_eq!(a.par_iter().min(), Some(&32));
    ///
    /// let b: [i32; 0] = [];
    ///
    /// assert_eq!(b.par_iter().min(), None);
    /// ```
    fn min(self) -> Option<Self::Item>
    where
        Self::Item: Ord,
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [-3_i32, 77, 53, 240, -1];
    ///
    /// assert_eq!(a.par_iter().min_by(|x, y| x.cmp(y)), Some(&-3));
    /// ```
    fn min_by<F>(self, f: F) -> Option<Self::Item>
    where
        F: Sync + Send + Fn(&Self::Item, &Self::Item) -> Ordering,
    {
        fn min<T>(f: impl Fn(&T, &T) -> Ordering) -> impl Fn(T, T) -> T {
            move |a, b| match f(&a, &b) {
                Ordering::Greater => b,
                _ => a,
            }
        }

        self.reduce_with(min(f))
    }

    /// Computes the item that yields the minimum value for the given
    /// function. If the iterator is empty, `None` is returned;
    /// otherwise, `Some(item)` is returned.
    ///
    /// Note that the order in which the items will be reduced is not
    /// specified, so if the `Ord` impl is not truly associative, then
    /// the results are not deterministic.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [-3_i32, 34, 2, 5, -10, -3, -23];
    ///
    /// assert_eq!(a.par_iter().min_by_key(|x| x.abs()), Some(&2));
    /// ```
    fn min_by_key<K, F>(self, f: F) -> Option<Self::Item>
    where
        K: Ord + Send,
        F: Sync + Send + Fn(&Self::Item) -> K,
    {
        fn key<T, K>(f: impl Fn(&T) -> K) -> impl Fn(T) -> (K, T) {
            move |x| (f(&x), x)
        }

        fn min_key<T, K: Ord>(a: (K, T), b: (K, T)) -> (K, T) {
            match (a.0).cmp(&b.0) {
                Ordering::Greater => b,
                _ => a,
            }
        }

        let (_, x) = self.map(key(f)).reduce_with(min_key)?;
        Some(x)
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [45, 74, 32];
    ///
    /// assert_eq!(a.par_iter().max(), Some(&74));
    ///
    /// let b: [i32; 0] = [];
    ///
    /// assert_eq!(b.par_iter().max(), None);
    /// ```
    fn max(self) -> Option<Self::Item>
    where
        Self::Item: Ord,
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [-3_i32, 77, 53, 240, -1];
    ///
    /// assert_eq!(a.par_iter().max_by(|x, y| x.abs().cmp(&y.abs())), Some(&240));
    /// ```
    fn max_by<F>(self, f: F) -> Option<Self::Item>
    where
        F: Sync + Send + Fn(&Self::Item, &Self::Item) -> Ordering,
    {
        fn max<T>(f: impl Fn(&T, &T) -> Ordering) -> impl Fn(T, T) -> T {
            move |a, b| match f(&a, &b) {
                Ordering::Greater => a,
                _ => b,
            }
        }

        self.reduce_with(max(f))
    }

    /// Computes the item that yields the maximum value for the given
    /// function. If the iterator is empty, `None` is returned;
    /// otherwise, `Some(item)` is returned.
    ///
    /// Note that the order in which the items will be reduced is not
    /// specified, so if the `Ord` impl is not truly associative, then
    /// the results are not deterministic.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [-3_i32, 34, 2, 5, -10, -3, -23];
    ///
    /// assert_eq!(a.par_iter().max_by_key(|x| x.abs()), Some(&34));
    /// ```
    fn max_by_key<K, F>(self, f: F) -> Option<Self::Item>
    where
        K: Ord + Send,
        F: Sync + Send + Fn(&Self::Item) -> K,
    {
        fn key<T, K>(f: impl Fn(&T) -> K) -> impl Fn(T) -> (K, T) {
            move |x| (f(&x), x)
        }

        fn max_key<T, K: Ord>(a: (K, T), b: (K, T)) -> (K, T) {
            match (a.0).cmp(&b.0) {
                Ordering::Greater => a,
                _ => b,
            }
        }

        let (_, x) = self.map(key(f)).reduce_with(max_key)?;
        Some(x)
    }

    /// Takes two iterators and creates a new iterator over both.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [0, 1, 2];
    /// let b = [9, 8, 7];
    ///
    /// let par_iter = a.par_iter().chain(b.par_iter());
    ///
    /// let chained: Vec<_> = par_iter.cloned().collect();
    ///
    /// assert_eq!(&chained[..], &[0, 1, 2, 9, 8, 7]);
    /// ```
    fn chain<C>(self, chain: C) -> Chain<Self, C::Iter>
    where
        C: IntoParallelIterator<Item = Self::Item>,
    {
        Chain::new(self, chain.into_par_iter())
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 2, 3, 3];
    ///
    /// assert_eq!(a.par_iter().find_any(|&&x| x == 3), Some(&3));
    ///
    /// assert_eq!(a.par_iter().find_any(|&&x| x == 100), None);
    /// ```
    fn find_any<P>(self, predicate: P) -> Option<Self::Item>
    where
        P: Fn(&Self::Item) -> bool + Sync + Send,
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 2, 3, 3];
    ///
    /// assert_eq!(a.par_iter().find_first(|&&x| x == 3), Some(&3));
    ///
    /// assert_eq!(a.par_iter().find_first(|&&x| x == 100), None);
    /// ```
    fn find_first<P>(self, predicate: P) -> Option<Self::Item>
    where
        P: Fn(&Self::Item) -> bool + Sync + Send,
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 2, 3, 3];
    ///
    /// assert_eq!(a.par_iter().find_last(|&&x| x == 3), Some(&3));
    ///
    /// assert_eq!(a.par_iter().find_last(|&&x| x == 100), None);
    /// ```
    fn find_last<P>(self, predicate: P) -> Option<Self::Item>
    where
        P: Fn(&Self::Item) -> bool + Sync + Send,
    {
        find_first_last::find_last(self, predicate)
    }

    /// Applies the given predicate to the items in the parallel iterator
    /// and returns **any** non-None result of the map operation.
    ///
    /// Once a non-None value is produced from the map operation, we will
    /// attempt to stop processing the rest of the items in the iterator
    /// as soon as possible.
    ///
    /// Note that this method only returns **some** item in the parallel
    /// iterator that is not None from the map predicate. The item returned
    /// may not be the **first** non-None value produced in the parallel
    /// sequence, since the entire sequence is mapped over in parallel.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let c = ["lol", "NaN", "5", "5"];
    ///
    /// let first_number = c.par_iter().find_map_first(|s| s.parse().ok());
    ///
    /// assert_eq!(first_number, Some(5));
    /// ```
    fn find_map_any<P, R>(self, predicate: P) -> Option<R>
    where
        P: Fn(Self::Item) -> Option<R> + Sync + Send,
        R: Send,
    {
        fn yes<T>(_: &T) -> bool {
            true
        }
        self.filter_map(predicate).find_any(yes)
    }

    /// Applies the given predicate to the items in the parallel iterator and
    /// returns the sequentially **first** non-None result of the map operation.
    ///
    /// Once a non-None value is produced from the map operation, all attempts
    /// to the right of the match will be stopped, while attempts to the left
    /// must continue in case an earlier match is found.
    ///
    /// Note that not all parallel iterators have a useful order, much like
    /// sequential `HashMap` iteration, so "first" may be nebulous. If you
    /// just want the first non-None value discovered anywhere in the iterator,
    /// `find_map_any` is a better choice.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let c = ["lol", "NaN", "2", "5"];
    ///
    /// let first_number = c.par_iter().find_map_first(|s| s.parse().ok());
    ///
    /// assert_eq!(first_number, Some(2));
    /// ```
    fn find_map_first<P, R>(self, predicate: P) -> Option<R>
    where
        P: Fn(Self::Item) -> Option<R> + Sync + Send,
        R: Send,
    {
        fn yes<T>(_: &T) -> bool {
            true
        }
        self.filter_map(predicate).find_first(yes)
    }

    /// Applies the given predicate to the items in the parallel iterator and
    /// returns the sequentially **last** non-None result of the map operation.
    ///
    /// Once a non-None value is produced from the map operation, all attempts
    /// to the left of the match will be stopped, while attempts to the right
    /// must continue in case a later match is found.
    ///
    /// Note that not all parallel iterators have a useful order, much like
    /// sequential `HashMap` iteration, so "first" may be nebulous. If you
    /// just want the first non-None value discovered anywhere in the iterator,
    /// `find_map_any` is a better choice.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let c = ["lol", "NaN", "2", "5"];
    ///
    /// let first_number = c.par_iter().find_map_last(|s| s.parse().ok());
    ///
    /// assert_eq!(first_number, Some(5));
    /// ```
    fn find_map_last<P, R>(self, predicate: P) -> Option<R>
    where
        P: Fn(Self::Item) -> Option<R> + Sync + Send,
        R: Send,
    {
        fn yes<T>(_: &T) -> bool {
            true
        }
        self.filter_map(predicate).find_last(yes)
    }

    #[doc(hidden)]
    #[deprecated(note = "parallel `find` does not search in order -- use `find_any`, \\
                         `find_first`, or `find_last`")]
    fn find<P>(self, predicate: P) -> Option<Self::Item>
    where
        P: Fn(&Self::Item) -> bool + Sync + Send,
    {
        self.find_any(predicate)
    }

    /// Searches for **some** item in the parallel iterator that
    /// matches the given predicate, and if so returns true.  Once
    /// a match is found, we'll attempt to stop process the rest
    /// of the items.  Proving that there's no match, returning false,
    /// does require visiting every item.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [0, 12, 3, 4, 0, 23, 0];
    ///
    /// let is_valid = a.par_iter().any(|&x| x > 10);
    ///
    /// assert!(is_valid);
    /// ```
    fn any<P>(self, predicate: P) -> bool
    where
        P: Fn(Self::Item) -> bool + Sync + Send,
    {
        self.map(predicate).find_any(bool::clone).is_some()
    }

    /// Tests that every item in the parallel iterator matches the given
    /// predicate, and if so returns true.  If a counter-example is found,
    /// we'll attempt to stop processing more items, then return false.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [0, 12, 3, 4, 0, 23, 0];
    ///
    /// let is_valid = a.par_iter().all(|&x| x > 10);
    ///
    /// assert!(!is_valid);
    /// ```
    fn all<P>(self, predicate: P) -> bool
    where
        P: Fn(Self::Item) -> bool + Sync + Send,
    {
        #[inline]
        fn is_false(x: &bool) -> bool {
            !x
        }

        self.map(predicate).find_any(is_false).is_none()
    }

    /// Creates an iterator over the `Some` items of this iterator, halting
    /// as soon as any `None` is found.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// let counter = AtomicUsize::new(0);
    /// let value = (0_i32..2048)
    ///     .into_par_iter()
    ///     .map(|x| {
    ///              counter.fetch_add(1, Ordering::SeqCst);
    ///              if x < 1024 { Some(x) } else { None }
    ///          })
    ///     .while_some()
    ///     .max();
    ///
    /// assert!(value < Some(1024));
    /// assert!(counter.load(Ordering::SeqCst) < 2048); // should not have visited every single one
    /// ```
    fn while_some<T>(self) -> WhileSome<Self>
    where
        Self: ParallelIterator<Item = Option<T>>,
        T: Send,
    {
        WhileSome::new(self)
    }

    /// Wraps an iterator with a fuse in case of panics, to halt all threads
    /// as soon as possible.
    ///
    /// Panics within parallel iterators are always propagated to the caller,
    /// but they don't always halt the rest of the iterator right away, due to
    /// the internal semantics of [`join`]. This adaptor makes a greater effort
    /// to stop processing other items sooner, with the cost of additional
    /// synchronization overhead, which may also inhibit some optimizations.
    ///
    /// [`join`]: ../fn.join.html#panics
    ///
    /// # Examples
    ///
    /// If this code didn't use `panic_fuse()`, it would continue processing
    /// many more items in other threads (with long sleep delays) before the
    /// panic is finally propagated.
    ///
    /// ```should_panic
    /// use rayon::prelude::*;
    /// use std::{thread, time};
    ///
    /// (0..1_000_000)
    ///     .into_par_iter()
    ///     .panic_fuse()
    ///     .for_each(|i| {
    ///         // simulate some work
    ///         thread::sleep(time::Duration::from_secs(1));
    ///         assert!(i > 0); // oops!
    ///     });
    /// ```
    fn panic_fuse(self) -> PanicFuse<Self> {
        PanicFuse::new(self)
    }

    /// Create a fresh collection containing all the element produced
    /// by this parallel iterator.
    ///
    /// You may prefer to use `collect_into_vec()`, which allocates more
    /// efficiently with precise knowledge of how many elements the
    /// iterator contains, and even allows you to reuse an existing
    /// vector's backing store rather than allocating a fresh vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let sync_vec: Vec<_> = (0..100).into_iter().collect();
    ///
    /// let async_vec: Vec<_> = (0..100).into_par_iter().collect();
    ///
    /// assert_eq!(sync_vec, async_vec);
    /// ```
    fn collect<C>(self) -> C
    where
        C: FromParallelIterator<Self::Item>,
    {
        C::from_par_iter(self)
    }

    /// Unzips the items of a parallel iterator into a pair of arbitrary
    /// `ParallelExtend` containers.
    ///
    /// You may prefer to use `unzip_into_vecs()`, which allocates more
    /// efficiently with precise knowledge of how many elements the
    /// iterator contains, and even allows you to reuse existing
    /// vectors' backing stores rather than allocating fresh vectors.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [(0, 1), (1, 2), (2, 3), (3, 4)];
    ///
    /// let (left, right): (Vec<_>, Vec<_>) = a.par_iter().cloned().unzip();
    ///
    /// assert_eq!(left, [0, 1, 2, 3]);
    /// assert_eq!(right, [1, 2, 3, 4]);
    /// ```
    ///
    /// Nested pairs can be unzipped too.
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let (values, (squares, cubes)): (Vec<_>, (Vec<_>, Vec<_>)) = (0..4).into_par_iter()
    ///     .map(|i| (i, (i * i, i * i * i)))
    ///     .unzip();
    ///
    /// assert_eq!(values, [0, 1, 2, 3]);
    /// assert_eq!(squares, [0, 1, 4, 9]);
    /// assert_eq!(cubes, [0, 1, 8, 27]);
    /// ```
    fn unzip<A, B, FromA, FromB>(self) -> (FromA, FromB)
    where
        Self: ParallelIterator<Item = (A, B)>,
        FromA: Default + Send + ParallelExtend<A>,
        FromB: Default + Send + ParallelExtend<B>,
        A: Send,
        B: Send,
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let (left, right): (Vec<_>, Vec<_>) = (0..8).into_par_iter().partition(|x| x % 2 == 0);
    ///
    /// assert_eq!(left, [0, 2, 4, 6]);
    /// assert_eq!(right, [1, 3, 5, 7]);
    /// ```
    fn partition<A, B, P>(self, predicate: P) -> (A, B)
    where
        A: Default + Send + ParallelExtend<Self::Item>,
        B: Default + Send + ParallelExtend<Self::Item>,
        P: Fn(&Self::Item) -> bool + Sync + Send,
    {
        unzip::partition(self, predicate)
    }

    /// Partitions and maps the items of a parallel iterator into a pair of
    /// arbitrary `ParallelExtend` containers.  `Either::Left` items go into
    /// the first container, and `Either::Right` items go into the second.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// use rayon::iter::Either;
    ///
    /// let (left, right): (Vec<_>, Vec<_>) = (0..8).into_par_iter()
    ///     .partition_map(|x| {
    ///         if x % 2 == 0 {
    ///             Either::Left(x * 4)
    ///         } else {
    ///             Either::Right(x * 3)
    ///         }
    ///     });
    ///
    /// assert_eq!(left, [0, 8, 16, 24]);
    /// assert_eq!(right, [3, 9, 15, 21]);
    /// ```
    ///
    /// Nested `Either` enums can be split as well.
    ///
    /// ```
    /// use rayon::prelude::*;
    /// use rayon::iter::Either::*;
    ///
    /// let ((fizzbuzz, fizz), (buzz, other)): ((Vec<_>, Vec<_>), (Vec<_>, Vec<_>)) = (1..20)
    ///     .into_par_iter()
    ///     .partition_map(|x| match (x % 3, x % 5) {
    ///         (0, 0) => Left(Left(x)),
    ///         (0, _) => Left(Right(x)),
    ///         (_, 0) => Right(Left(x)),
    ///         (_, _) => Right(Right(x)),
    ///     });
    ///
    /// assert_eq!(fizzbuzz, [15]);
    /// assert_eq!(fizz, [3, 6, 9, 12, 18]);
    /// assert_eq!(buzz, [5, 10]);
    /// assert_eq!(other, [1, 2, 4, 7, 8, 11, 13, 14, 16, 17, 19]);
    /// ```
    fn partition_map<A, B, P, L, R>(self, predicate: P) -> (A, B)
    where
        A: Default + Send + ParallelExtend<L>,
        B: Default + Send + ParallelExtend<R>,
        P: Fn(Self::Item) -> Either<L, R> + Sync + Send,
        L: Send,
        R: Send,
    {
        unzip::partition_map(self, predicate)
    }

    /// Intersperses clones of an element between items of this iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let x = vec![1, 2, 3];
    /// let r: Vec<_> = x.into_par_iter().intersperse(-1).collect();
    ///
    /// assert_eq!(r, vec![1, -1, 2, -1, 3]);
    /// ```
    fn intersperse(self, element: Self::Item) -> Intersperse<Self>
    where
        Self::Item: Clone,
    {
        Intersperse::new(self, element)
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
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>;

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
    fn opt_len(&self) -> Option<usize> {
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
///
/// **Note:** Not implemented for `u64`, `i64`, `u128`, or `i128` ranges
pub trait IndexedParallelIterator: ParallelIterator {
    /// Sets the "scheduler" that this thread-pool will use.
    fn with_scheduler<S>(self, scheduler: S) -> WithScheduler<Self, S> {
        WithScheduler::new(self, scheduler)
    }

    /// Collects the results of the iterator into the specified
    /// vector. The vector is always truncated before execution
    /// begins. If possible, reusing the vector across calls can lead
    /// to better performance since it reuses the same backing buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// // any prior data will be truncated
    /// let mut vec = vec![-1, -2, -3];
    ///
    /// (0..5).into_par_iter()
    ///     .collect_into_vec(&mut vec);
    ///
    /// assert_eq!(vec, [0, 1, 2, 3, 4]);
    /// ```
    fn collect_into_vec(self, target: &mut Vec<Self::Item>) {
        collect::collect_into_vec(self, target);
    }

    /// Unzips the results of the iterator into the specified
    /// vectors. The vectors are always truncated before execution
    /// begins. If possible, reusing the vectors across calls can lead
    /// to better performance since they reuse the same backing buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// // any prior data will be truncated
    /// let mut left = vec![42; 10];
    /// let mut right = vec![-1; 10];
    ///
    /// (10..15).into_par_iter()
    ///     .enumerate()
    ///     .unzip_into_vecs(&mut left, &mut right);
    ///
    /// assert_eq!(left, [0, 1, 2, 3, 4]);
    /// assert_eq!(right, [10, 11, 12, 13, 14]);
    /// ```
    fn unzip_into_vecs<A, B>(self, left: &mut Vec<A>, right: &mut Vec<B>)
    where
        Self: IndexedParallelIterator<Item = (A, B)>,
        A: Send,
        B: Send,
    {
        collect::unzip_into_vecs(self, left, right);
    }

    /// Iterate over tuples `(A, B)`, where the items `A` are from
    /// this iterator and `B` are from the iterator given as argument.
    /// Like the `zip` method on ordinary iterators, if the two
    /// iterators are of unequal length, you only get the items they
    /// have in common.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let result: Vec<_> = (1..4)
    ///     .into_par_iter()
    ///     .zip(vec!['a', 'b', 'c'])
    ///     .collect();
    ///
    /// assert_eq!(result, [(1, 'a'), (2, 'b'), (3, 'c')]);
    /// ```
    fn zip<Z>(self, zip_op: Z) -> Zip<Self, Z::Iter>
    where
        Z: IntoParallelIterator,
        Z::Iter: IndexedParallelIterator,
    {
        Zip::new(self, zip_op.into_par_iter())
    }

    /// The same as `Zip`, but requires that both iterators have the same length.
    ///
    /// # Panics
    /// Will panic if `self` and `zip_op` are not the same length.
    ///
    /// ```should_panic
    /// use rayon::prelude::*;
    ///
    /// let one = [1u8];
    /// let two = [2u8, 2];
    /// let one_iter = one.par_iter();
    /// let two_iter = two.par_iter();
    ///
    /// // this will panic
    /// let zipped: Vec<(&u8, &u8)> = one_iter.zip_eq(two_iter).collect();
    ///
    /// // we should never get here
    /// assert_eq!(1, zipped.len());
    /// ```
    fn zip_eq<Z>(self, zip_op: Z) -> ZipEq<Self, Z::Iter>
    where
        Z: IntoParallelIterator,
        Z::Iter: IndexedParallelIterator,
    {
        let zip_op_iter = zip_op.into_par_iter();
        assert_eq!(self.len(), zip_op_iter.len());
        ZipEq::new(self, zip_op_iter)
    }

    /// Interleave elements of this iterator and the other given
    /// iterator. Alternately yields elements from this iterator and
    /// the given iterator, until both are exhausted. If one iterator
    /// is exhausted before the other, the last elements are provided
    /// from the other.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// let (x, y) = (vec![1, 2], vec![3, 4, 5, 6]);
    /// let r: Vec<i32> = x.into_par_iter().interleave(y).collect();
    /// assert_eq!(r, vec![1, 3, 2, 4, 5, 6]);
    /// ```
    fn interleave<I>(self, other: I) -> Interleave<Self, I::Iter>
    where
        I: IntoParallelIterator<Item = Self::Item>,
        I::Iter: IndexedParallelIterator<Item = Self::Item>,
    {
        Interleave::new(self, other.into_par_iter())
    }

    /// Interleave elements of this iterator and the other given
    /// iterator, until one is exhausted.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// let (x, y) = (vec![1, 2, 3, 4], vec![5, 6]);
    /// let r: Vec<i32> = x.into_par_iter().interleave_shortest(y).collect();
    /// assert_eq!(r, vec![1, 5, 2, 6, 3]);
    /// ```
    fn interleave_shortest<I>(self, other: I) -> InterleaveShortest<Self, I::Iter>
    where
        I: IntoParallelIterator<Item = Self::Item>,
        I::Iter: IndexedParallelIterator<Item = Self::Item>,
    {
        InterleaveShortest::new(self, other.into_par_iter())
    }

    /// Split an iterator up into fixed-size chunks.
    ///
    /// Returns an iterator that returns `Vec`s of the given number of elements.
    /// If the number of elements in the iterator is not divisible by `chunk_size`,
    /// the last chunk may be shorter than `chunk_size`.
    ///
    /// See also [`par_chunks()`] and [`par_chunks_mut()`] for similar behavior on
    /// slices, without having to allocate intermediate `Vec`s for the chunks.
    ///
    /// [`par_chunks()`]: ../slice/trait.ParallelSlice.html#method.par_chunks
    /// [`par_chunks_mut()`]: ../slice/trait.ParallelSliceMut.html#method.par_chunks_mut
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// let a = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    /// let r: Vec<Vec<i32>> = a.into_par_iter().chunks(3).collect();
    /// assert_eq!(r, vec![vec![1,2,3], vec![4,5,6], vec![7,8,9], vec![10]]);
    /// ```
    fn chunks(self, chunk_size: usize) -> Chunks<Self> {
        assert!(chunk_size != 0, "chunk_size must not be zero");
        Chunks::new(self, chunk_size)
    }

    /// Lexicographically compares the elements of this `ParallelIterator` with those of
    /// another.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// use std::cmp::Ordering::*;
    ///
    /// let x = vec![1, 2, 3];
    /// assert_eq!(x.par_iter().cmp(&vec![1, 3, 0]), Less);
    /// assert_eq!(x.par_iter().cmp(&vec![1, 2, 3]), Equal);
    /// assert_eq!(x.par_iter().cmp(&vec![1, 2]), Greater);
    /// ```
    fn cmp<I>(self, other: I) -> Ordering
    where
        I: IntoParallelIterator<Item = Self::Item>,
        I::Iter: IndexedParallelIterator,
        Self::Item: Ord,
    {
        #[inline]
        fn ordering<T: Ord>((x, y): (T, T)) -> Ordering {
            Ord::cmp(&x, &y)
        }

        #[inline]
        fn inequal(&ord: &Ordering) -> bool {
            ord != Ordering::Equal
        }

        let other = other.into_par_iter();
        let ord_len = self.len().cmp(&other.len());
        self.zip(other)
            .map(ordering)
            .find_first(inequal)
            .unwrap_or(ord_len)
    }

    /// Lexicographically compares the elements of this `ParallelIterator` with those of
    /// another.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    /// use std::cmp::Ordering::*;
    /// use std::f64::NAN;
    ///
    /// let x = vec![1.0, 2.0, 3.0];
    /// assert_eq!(x.par_iter().partial_cmp(&vec![1.0, 3.0, 0.0]), Some(Less));
    /// assert_eq!(x.par_iter().partial_cmp(&vec![1.0, 2.0, 3.0]), Some(Equal));
    /// assert_eq!(x.par_iter().partial_cmp(&vec![1.0, 2.0]), Some(Greater));
    /// assert_eq!(x.par_iter().partial_cmp(&vec![1.0, NAN]), None);
    /// ```
    fn partial_cmp<I>(self, other: I) -> Option<Ordering>
    where
        I: IntoParallelIterator,
        I::Iter: IndexedParallelIterator,
        Self::Item: PartialOrd<I::Item>,
    {
        #[inline]
        fn ordering<T: PartialOrd<U>, U>((x, y): (T, U)) -> Option<Ordering> {
            PartialOrd::partial_cmp(&x, &y)
        }

        #[inline]
        fn inequal(&ord: &Option<Ordering>) -> bool {
            ord != Some(Ordering::Equal)
        }

        let other = other.into_par_iter();
        let ord_len = self.len().cmp(&other.len());
        self.zip(other)
            .map(ordering)
            .find_first(inequal)
            .unwrap_or(Some(ord_len))
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are equal to those of another
    fn eq<I>(self, other: I) -> bool
    where
        I: IntoParallelIterator,
        I::Iter: IndexedParallelIterator,
        Self::Item: PartialEq<I::Item>,
    {
        #[inline]
        fn eq<T: PartialEq<U>, U>((x, y): (T, U)) -> bool {
            PartialEq::eq(&x, &y)
        }

        let other = other.into_par_iter();
        self.len() == other.len() && self.zip(other).all(eq)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are unequal to those of another
    fn ne<I>(self, other: I) -> bool
    where
        I: IntoParallelIterator,
        I::Iter: IndexedParallelIterator,
        Self::Item: PartialEq<I::Item>,
    {
        !self.eq(other)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are lexicographically less than those of another.
    fn lt<I>(self, other: I) -> bool
    where
        I: IntoParallelIterator,
        I::Iter: IndexedParallelIterator,
        Self::Item: PartialOrd<I::Item>,
    {
        self.partial_cmp(other) == Some(Ordering::Less)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are less or equal to those of another.
    fn le<I>(self, other: I) -> bool
    where
        I: IntoParallelIterator,
        I::Iter: IndexedParallelIterator,
        Self::Item: PartialOrd<I::Item>,
    {
        let ord = self.partial_cmp(other);
        ord == Some(Ordering::Equal) || ord == Some(Ordering::Less)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are lexicographically greater than those of another.
    fn gt<I>(self, other: I) -> bool
    where
        I: IntoParallelIterator,
        I::Iter: IndexedParallelIterator,
        Self::Item: PartialOrd<I::Item>,
    {
        self.partial_cmp(other) == Some(Ordering::Greater)
    }

    /// Determines if the elements of this `ParallelIterator`
    /// are less or equal to those of another.
    fn ge<I>(self, other: I) -> bool
    where
        I: IntoParallelIterator,
        I::Iter: IndexedParallelIterator,
        Self::Item: PartialOrd<I::Item>,
    {
        let ord = self.partial_cmp(other);
        ord == Some(Ordering::Equal) || ord == Some(Ordering::Greater)
    }

    /// Yields an index along with each item.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let chars = vec!['a', 'b', 'c'];
    /// let result: Vec<_> = chars
    ///     .into_par_iter()
    ///     .enumerate()
    ///     .collect();
    ///
    /// assert_eq!(result, [(0, 'a'), (1, 'b'), (2, 'c')]);
    /// ```
    fn enumerate(self) -> Enumerate<Self> {
        Enumerate::new(self)
    }

    /// Creates an iterator that steps by the given amount
    ///
    /// # Examples
    ///
    /// ```
    ///use rayon::prelude::*;
    ///
    /// let range = (3..10);
    /// let result: Vec<i32> = range
    ///    .into_par_iter()
    ///    .step_by(3)
    ///    .collect();
    ///
    /// assert_eq!(result, [3, 6, 9])
    /// ```
    ///
    /// # Compatibility
    ///
    /// This method is only available on Rust 1.38 or greater.
    #[cfg(step_by)]
    fn step_by(self, step: usize) -> StepBy<Self> {
        StepBy::new(self, step)
    }

    /// Creates an iterator that skips the first `n` elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let result: Vec<_> = (0..100)
    ///     .into_par_iter()
    ///     .skip(95)
    ///     .collect();
    ///
    /// assert_eq!(result, [95, 96, 97, 98, 99]);
    /// ```
    fn skip(self, n: usize) -> Skip<Self> {
        Skip::new(self, n)
    }

    /// Creates an iterator that yields the first `n` elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let result: Vec<_> = (0..100)
    ///     .into_par_iter()
    ///     .take(5)
    ///     .collect();
    ///
    /// assert_eq!(result, [0, 1, 2, 3, 4]);
    /// ```
    fn take(self, n: usize) -> Take<Self> {
        Take::new(self, n)
    }

    /// Searches for **some** item in the parallel iterator that
    /// matches the given predicate, and returns its index.  Like
    /// `ParallelIterator::find_any`, the parallel search will not
    /// necessarily find the **first** match, and once a match is
    /// found we'll attempt to stop processing any more.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 2, 3, 3];
    ///
    /// let i = a.par_iter().position_any(|&x| x == 3).expect("found");
    /// assert!(i == 2 || i == 3);
    ///
    /// assert_eq!(a.par_iter().position_any(|&x| x == 100), None);
    /// ```
    fn position_any<P>(self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool + Sync + Send,
    {
        #[inline]
        fn check(&(_, p): &(usize, bool)) -> bool {
            p
        }

        let (i, _) = self.map(predicate).enumerate().find_any(check)?;
        Some(i)
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 2, 3, 3];
    ///
    /// assert_eq!(a.par_iter().position_first(|&x| x == 3), Some(2));
    ///
    /// assert_eq!(a.par_iter().position_first(|&x| x == 100), None);
    /// ```
    fn position_first<P>(self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool + Sync + Send,
    {
        #[inline]
        fn check(&(_, p): &(usize, bool)) -> bool {
            p
        }

        let (i, _) = self.map(predicate).enumerate().find_first(check)?;
        Some(i)
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
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let a = [1, 2, 3, 3];
    ///
    /// assert_eq!(a.par_iter().position_last(|&x| x == 3), Some(3));
    ///
    /// assert_eq!(a.par_iter().position_last(|&x| x == 100), None);
    /// ```
    fn position_last<P>(self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool + Sync + Send,
    {
        #[inline]
        fn check(&(_, p): &(usize, bool)) -> bool {
            p
        }

        let (i, _) = self.map(predicate).enumerate().find_last(check)?;
        Some(i)
    }

    #[doc(hidden)]
    #[deprecated(
        note = "parallel `position` does not search in order -- use `position_any`, \\
                `position_first`, or `position_last`"
    )]
    fn position<P>(self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool + Sync + Send,
    {
        self.position_any(predicate)
    }

    /// Produces a new iterator with the elements of this iterator in
    /// reverse order.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let result: Vec<_> = (0..5)
    ///     .into_par_iter()
    ///     .rev()
    ///     .collect();
    ///
    /// assert_eq!(result, [4, 3, 2, 1, 0]);
    /// ```
    fn rev(self) -> Rev<Self> {
        Rev::new(self)
    }

    /// Sets the minimum length of iterators desired to process in each
    /// thread.  Rayon will not split any smaller than this length, but
    /// of course an iterator could already be smaller to begin with.
    ///
    /// Producers like `zip` and `interleave` will use greater of the two
    /// minimums.
    /// Chained iterators and iterators inside `flat_map` may each use
    /// their own minimum length.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let min = (0..1_000_000)
    ///     .into_par_iter()
    ///     .with_min_len(1234)
    ///     .fold(|| 0, |acc, _| acc + 1) // count how many are in this segment
    ///     .min().unwrap();
    ///
    /// assert!(min >= 1234);
    /// ```
    fn with_min_len(self, min: usize) -> MinLen<Self> {
        MinLen::new(self, min)
    }

    /// Sets the maximum length of iterators desired to process in each
    /// thread.  Rayon will try to split at least below this length,
    /// unless that would put it below the length from `with_min_len()`.
    /// For example, given min=10 and max=15, a length of 16 will not be
    /// split any further.
    ///
    /// Producers like `zip` and `interleave` will use lesser of the two
    /// maximums.
    /// Chained iterators and iterators inside `flat_map` may each use
    /// their own maximum length.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let max = (0..1_000_000)
    ///     .into_par_iter()
    ///     .with_max_len(1234)
    ///     .fold(|| 0, |acc, _| acc + 1) // count how many are in this segment
    ///     .max().unwrap();
    ///
    /// assert!(max <= 1234);
    /// ```
    fn with_max_len(self, max: usize) -> MaxLen<Self> {
        MaxLen::new(self, max)
    }

    /// Produces an exact count of how many items this iterator will
    /// produce, presuming no panic occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let par_iter = (0..100).into_par_iter().zip(vec![0; 10]);
    /// assert_eq!(par_iter.len(), 10);
    ///
    /// let vec: Vec<_> = par_iter.collect();
    /// assert_eq!(vec.len(), 10);
    /// ```
    fn len(&self) -> usize;

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
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result;

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

/// `FromParallelIterator` implements the creation of a collection
/// from a [`ParallelIterator`]. By implementing
/// `FromParallelIterator` for a given type, you define how it will be
/// created from an iterator.
///
/// `FromParallelIterator` is used through [`ParallelIterator`]'s [`collect()`] method.
///
/// [`ParallelIterator`]: trait.ParallelIterator.html
/// [`collect()`]: trait.ParallelIterator.html#method.collect
///
/// # Examples
///
/// Implementing `FromParallelIterator` for your type:
///
/// ```
/// use rayon::prelude::*;
/// use std::mem;
///
/// struct BlackHole {
///     mass: usize,
/// }
///
/// impl<T: Send> FromParallelIterator<T> for BlackHole {
///     fn from_par_iter<I>(par_iter: I) -> Self
///         where I: IntoParallelIterator<Item = T>
///     {
///         let par_iter = par_iter.into_par_iter();
///         BlackHole {
///             mass: par_iter.count() * mem::size_of::<T>(),
///         }
///     }
/// }
///
/// let bh: BlackHole = (0i32..1000).into_par_iter().collect();
/// assert_eq!(bh.mass, 4000);
/// ```
pub trait FromParallelIterator<T>
where
    T: Send,
{
    /// Creates an instance of the collection from the parallel iterator `par_iter`.
    ///
    /// If your collection is not naturally parallel, the easiest (and
    /// fastest) way to do this is often to collect `par_iter` into a
    /// [`LinkedList`] or other intermediate data structure and then
    /// sequentially extend your collection. However, a more 'native'
    /// technique is to use the [`par_iter.fold`] or
    /// [`par_iter.fold_with`] methods to create the collection.
    /// Alternatively, if your collection is 'natively' parallel, you
    /// can use `par_iter.for_each` to process each element in turn.
    ///
    /// [`LinkedList`]: https://doc.rust-lang.org/std/collections/struct.LinkedList.html
    /// [`par_iter.fold`]: trait.ParallelIterator.html#method.fold
    /// [`par_iter.fold_with`]: trait.ParallelIterator.html#method.fold_with
    /// [`par_iter.for_each`]: trait.ParallelIterator.html#method.for_each
    fn from_par_iter<I>(par_iter: I) -> Self
    where
        I: IntoParallelIterator<Item = T>;
}

/// `ParallelExtend` extends an existing collection with items from a [`ParallelIterator`].
///
/// [`ParallelIterator`]: trait.ParallelIterator.html
///
/// # Examples
///
/// Implementing `ParallelExtend` for your type:
///
/// ```
/// use rayon::prelude::*;
/// use std::mem;
///
/// struct BlackHole {
///     mass: usize,
/// }
///
/// impl<T: Send> ParallelExtend<T> for BlackHole {
///     fn par_extend<I>(&mut self, par_iter: I)
///         where I: IntoParallelIterator<Item = T>
///     {
///         let par_iter = par_iter.into_par_iter();
///         self.mass += par_iter.count() * mem::size_of::<T>();
///     }
/// }
///
/// let mut bh = BlackHole { mass: 0 };
/// bh.par_extend(0i32..1000);
/// assert_eq!(bh.mass, 4000);
/// bh.par_extend(0i64..10);
/// assert_eq!(bh.mass, 4080);
/// ```
pub trait ParallelExtend<T>
where
    T: Send,
{
    /// Extends an instance of the collection with the elements drawn
    /// from the parallel iterator `par_iter`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rayon::prelude::*;
    ///
    /// let mut vec = vec![];
    /// vec.par_extend(0..5);
    /// vec.par_extend((0..5).into_par_iter().map(|i| i * i));
    /// assert_eq!(vec, [0, 1, 2, 3, 4, 0, 1, 4, 9, 16]);
    /// ```
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = T>;
}

/// We hide the `Try` trait in a private module, as it's only meant to be a
/// stable clone of the standard library's `Try` trait, as yet unstable.
mod private {
    /// Clone of `std::ops::Try`.
    ///
    /// Implementing this trait is not permitted outside of `rayon`.
    pub trait Try {
        private_decl! {}

        type Ok;
        type Error;
        fn into_result(self) -> Result<Self::Ok, Self::Error>;
        fn from_ok(v: Self::Ok) -> Self;
        fn from_error(v: Self::Error) -> Self;
    }

    impl<T> Try for Option<T> {
        private_impl! {}

        type Ok = T;
        type Error = ();

        fn into_result(self) -> Result<T, ()> {
            self.ok_or(())
        }
        fn from_ok(v: T) -> Self {
            Some(v)
        }
        fn from_error(_: ()) -> Self {
            None
        }
    }

    impl<T, E> Try for Result<T, E> {
        private_impl! {}

        type Ok = T;
        type Error = E;

        fn into_result(self) -> Result<T, E> {
            self
        }
        fn from_ok(v: T) -> Self {
            Ok(v)
        }
        fn from_error(v: E) -> Self {
            Err(v)
        }
    }
}
