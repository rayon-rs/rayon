//! Parallel iterator types for [inclusive ranges][std::range],
//! the type for values created by `a..=b` expressions
//!
//! You will rarely need to interact with this module directly unless you have
//! need to name one of the iterator types.
//!
//! ```
//! use rayon::prelude::*;
//!
//! let r = (0..=100u64).into_par_iter()
//!                     .sum();
//!
//! // compare result with sequential calculation
//! assert_eq!((0..=100).sum::<u64>(), r);
//! ```
//!
//! [std::range]: https://doc.rust-lang.org/core/ops/struct.RangeInclusive.html

use iter::plumbing::*;
use iter::*;
use std::ops::RangeInclusive;

/// Parallel iterator over an inclusive range, implemented for all integer types.
///
/// **Note:** The `zip` operation requires `IndexedParallelIterator`
/// which is only implemented for `u8`, `i8`, `u16`, and `i16`.
///
/// ```
/// use rayon::prelude::*;
///
/// let p = (0..=25u16).into_par_iter()
///                   .zip(0..=25u16)
///                   .filter(|&(x, y)| x % 5 == 0 || y % 5 == 0)
///                   .map(|(x, y)| x * y)
///                   .sum::<u16>();
///
/// let s = (0..=25u16).zip(0..=25u16)
///                   .filter(|&(x, y)| x % 5 == 0 || y % 5 == 0)
///                   .map(|(x, y)| x * y)
///                   .sum();
///
/// assert_eq!(p, s);
/// ```
#[derive(Debug, Clone)]
pub struct Iter<T> {
    range: RangeInclusive<T>,
}

impl<T> Iter<T>
where
    RangeInclusive<T>: Clone + Iterator<Item = T> + DoubleEndedIterator,
{
    /// Returns `Some((start, end))` for `start..=end`, or `None` if it is exhausted.
    ///
    /// Note that `RangeInclusive` does not specify the bounds of an exhausted iterator,
    /// so this is a way for us to figure out what we've got.  Thankfully, all of the
    /// integer types we care about can be trivially cloned.
    fn bounds(&self) -> Option<(T, T)> {
        Some((self.range.clone().next()?, self.range.clone().next_back()?))
    }
}

impl<T> IntoParallelIterator for RangeInclusive<T>
where
    Iter<T>: ParallelIterator,
{
    type Item = <Iter<T> as ParallelIterator>::Item;
    type Iter = Iter<T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { range: self }
    }
}

macro_rules! convert {
    ( $self:ident . $method:ident ( $( $arg:expr ),* ) ) => {
        if let Some((start, end)) = $self.bounds() {
            if let Some(end) = end.checked_add(1) {
                (start..end).into_par_iter().$method($( $arg ),*)
            } else {
                (start..end).into_par_iter().chain(once(end)).$method($( $arg ),*)
            }
        } else {
            empty::<Self::Item>().$method($( $arg ),*)
        }
    };
}

macro_rules! parallel_range_impl {
    ( $t:ty ) => {
        impl ParallelIterator for Iter<$t> {
            type Item = $t;

            fn drive_unindexed<C>(self, consumer: C) -> C::Result
            where
                C: UnindexedConsumer<Self::Item>,
            {
                convert!(self.drive_unindexed(consumer))
            }

            fn opt_len(&self) -> Option<usize> {
                convert!(self.opt_len())
            }
        }
    };
}

macro_rules! indexed_range_impl {
    ( $t:ty ) => {
        parallel_range_impl! { $t }

        impl IndexedParallelIterator for Iter<$t> {
            fn drive<C>(self, consumer: C) -> C::Result
            where
                C: Consumer<Self::Item>,
            {
                convert!(self.drive(consumer))
            }

            fn len(&self) -> usize {
                self.range.len()
            }

            fn with_producer<CB>(self, callback: CB) -> CB::Output
            where
                CB: ProducerCallback<Self::Item>,
            {
                convert!(self.with_producer(callback))
            }
        }
    };
}

// all RangeInclusive<T> with ExactSizeIterator
indexed_range_impl! {u8}
indexed_range_impl! {u16}
indexed_range_impl! {i8}
indexed_range_impl! {i16}

// other RangeInclusive<T> with just Iterator
parallel_range_impl! {usize}
parallel_range_impl! {isize}
parallel_range_impl! {u32}
parallel_range_impl! {i32}
parallel_range_impl! {u64}
parallel_range_impl! {i64}
parallel_range_impl! {u128}
parallel_range_impl! {i128}

#[test]
#[cfg(target_pointer_width = "64")]
fn test_u32_opt_len() {
    use std::u32;
    assert_eq!(Some(101), (0..=100u32).into_par_iter().opt_len());
    assert_eq!(
        Some(u32::MAX as usize),
        (0..=u32::MAX - 1).into_par_iter().opt_len()
    );
    assert_eq!(
        Some(u32::MAX as usize + 1),
        (0..=u32::MAX).into_par_iter().opt_len()
    );
}

#[test]
fn test_u64_opt_len() {
    use std::{u64, usize};
    assert_eq!(Some(101), (0..=100u64).into_par_iter().opt_len());
    assert_eq!(
        Some(usize::MAX),
        (0..=usize::MAX as u64 - 1).into_par_iter().opt_len()
    );
    assert_eq!(None, (0..=usize::MAX as u64).into_par_iter().opt_len());
    assert_eq!(None, (0..=u64::MAX).into_par_iter().opt_len());
}

#[test]
fn test_u128_opt_len() {
    use std::{u128, usize};
    assert_eq!(Some(101), (0..=100u128).into_par_iter().opt_len());
    assert_eq!(
        Some(usize::MAX),
        (0..=usize::MAX as u128 - 1).into_par_iter().opt_len()
    );
    assert_eq!(None, (0..=usize::MAX as u128).into_par_iter().opt_len());
    assert_eq!(None, (0..=u128::MAX).into_par_iter().opt_len());
}

// `usize as i64` can overflow, so make sure to wrap it appropriately
// when using the `opt_len` "indexed" mode.
#[test]
#[cfg(target_pointer_width = "64")]
fn test_usize_i64_overflow() {
    use std::i64;
    use ThreadPoolBuilder;

    let iter = (-2..=i64::MAX).into_par_iter();
    assert_eq!(iter.opt_len(), Some(i64::MAX as usize + 3));

    // always run with multiple threads to split into, or this will take forever...
    let pool = ThreadPoolBuilder::new().num_threads(8).build().unwrap();
    pool.install(|| assert_eq!(iter.find_last(|_| true), Some(i64::MAX)));
}
