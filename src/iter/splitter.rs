use super::plumbing::*;
use super::*;

use std::fmt::{self, Debug};

/// The `split` function takes arbitrary data and a closure that knows how to
/// split it, and turns this into a `ParallelIterator`.
///
/// # Examples
///
/// ```
/// use rayon::iter;
/// use rayon::prelude::*;
/// use std::ops::Range;
///
///
/// // This is a range of bi-dimensional indices
/// #[derive(Debug)]
/// struct Range2D {
///     // Horizontal range of indices
///     pub rx: Range<usize>,
///
///     // Vertical range of indices
///     pub ry: Range<usize>,
/// }
///
/// // We want to recursively split it by the largest dimension until we have
/// // enough sub-ranges to feed our mighty multi-core CPU. This function
/// // carries out one such split.
/// fn split_range(r2: Range2D) -> (Range2D, Option<Range2D>) {
///     // Decide on which axis (horizontal/vertical) the range should be split
///     let width = r2.rx.end - r2.rx.start;
///     let height = r2.ry.end - r2.ry.start;
///     if width >= height {
///         // We are mathematically unable to split the range if there is only
///         // one point inside of it, but we could stop splitting before that.
///         if width <= 1 { return (r2, None); }
///
///         // Here, our range is considered large enough to be splittable on
///         // the horizontal axis
///         let midpoint = r2.rx.start + (r2.rx.end - r2.rx.start) / 2;
///         let out1 = Range2D {
///             rx: Range {
///                 start: r2.rx.start,
///                 end: midpoint,
///             },
///             ry: r2.ry.clone(),
///         };
///         let out2 = Range2D {
///             rx: Range {
///                 start: midpoint,
///                 end: r2.rx.end,
///             },
///             ry: r2.ry,
///         };
///         (out1, Some(out2))
///     } else {
///         // This is essentially the same work as above, but vertically
///         if height <= 1 { return (r2, None); }
///         let midpoint = r2.ry.start + (r2.ry.end - r2.ry.start) / 2;
///         let out1 = Range2D {
///             rx: r2.rx.clone(),
///             ry: Range {
///                 start: r2.ry.start,
///                 end: midpoint,
///             },
///         };
///         let out2 = Range2D {
///             rx: r2.rx,
///             ry: Range {
///                 start: midpoint,
///                 end: r2.ry.end,
///             },
///         };
///         (out1, Some(out2))
///     }
/// }
///
/// // By using iter::split, Rayon will gladly do the recursive splitting for us
/// let range = Range2D { rx: 0..800, ry: 0..600 };
/// iter::split(range, split_range).for_each(|sub_range| {
///     // If the sub-ranges were indeed split by the largest dimension, then
///     // if no dimension was twice larger than the other initially, this
///     // property will remain true in the final sub-ranges.
///     let width = sub_range.rx.end - sub_range.rx.start;
///     let height = sub_range.ry.end - sub_range.ry.start;
///     assert!((width < 2 * height) && (height < 2 * width));
/// });
/// ```
///
pub fn split<D, S>(data: D, splitter: S) -> Split<D, S>
    where D: Send,
          S: Fn(D) -> (D, Option<D>) + Sync
{
    Split {
        data: data,
        splitter: splitter,
    }
}

/// `Split` is a parallel iterator using arbitrary data and a splitting function.
/// This struct is created by the [`split()`] function.
///
/// [`split()`]: fn.split.html
#[derive(Clone)]
pub struct Split<D, S> {
    data: D,
    splitter: S,
}

impl<D: Debug, S> Debug for Split<D, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Split")
            .field("data", &self.data)
            .finish()
    }
}

impl<D, S> ParallelIterator for Split<D, S>
    where D: Send,
          S: Fn(D) -> (D, Option<D>) + Sync + Send
{
    type Item = D;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitProducer {
            data: self.data,
            splitter: &self.splitter,
        };
        bridge_unindexed(producer, consumer)
    }
}

struct SplitProducer<'a, D, S: 'a> {
    data: D,
    splitter: &'a S,
}

impl<'a, D, S> UnindexedProducer for SplitProducer<'a, D, S>
    where D: Send,
          S: Fn(D) -> (D, Option<D>) + Sync
{
    type Item = D;

    fn split(mut self) -> (Self, Option<Self>) {
        let splitter = self.splitter;
        let (left, right) = splitter(self.data);
        self.data = left;
        (self,
         right.map(|data| {
                       SplitProducer {
                           data: data,
                           splitter: splitter,
                       }
                   }))
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        folder.consume(self.data)
    }
}
