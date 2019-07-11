use super::plumbing::*;
use super::*;

/// `Flatten` turns each element to an iterator, then flattens these iterators
/// together. This struct is created by the [`flatten()`] method on
/// [`ParallelIterator`].
///
/// [`flatten()`]: trait.ParallelIterator.html#method.flatten
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct Flatten<I: ParallelIterator> {
    base: I,
}

impl<I, PI> Flatten<I>
where
    I: ParallelIterator<Item = PI>,
    PI: IntoParallelIterator + Send,
{
    /// Create a new `Flatten` iterator.
    pub(super) fn new(base: I) -> Self {
        Flatten { base }
    }
}

impl<I, PI> ParallelIterator for Flatten<I>
where
    I: ParallelIterator<Item = PI>,
    PI: IntoParallelIterator + Send,
{
    type Item = PI::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        fn id<T>(x: T) -> T {
            x
        }

        self.base.flat_map(id).drive_unindexed(consumer)
    }
}
