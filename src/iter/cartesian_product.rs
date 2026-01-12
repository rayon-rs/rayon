use super::plumbing::*;
use super::repeat::*;
use super::{IndexedParallelIterator, ParallelIterator};

/// `CartesianProduct` is an iterator that combines `i` and `j` into a single iterator that
/// iterates over the cartesian product of the elements of `i` and `j`. This struct is created by
/// the [`cartesian_product()`] method on [`ParallelIterator`]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone, Copy)]
pub struct CartesianProduct<I, J>
where
    I: ParallelIterator,
    J: ParallelIterator,
{
    i: I,
    j: J,
}

impl<I, J> CartesianProduct<I, J>
where
    I: ParallelIterator,
    J: IndexedParallelIterator,
{
    /// Creates a new `CartesianProduct` iterator.
    pub(super) fn new(i: I, j: J) -> Self {
        CartesianProduct { i, j }
    }
}

impl<I, J> ParallelIterator for CartesianProduct<I, J>
where
    I: ParallelIterator,
    J: IndexedParallelIterator + Clone + Sync,
    I::Item: Clone,
{
    type Item = (I::Item, J::Item);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.i
            .flat_map(|i_item| repeat(i_item).zip(self.j.clone()))
            .drive_unindexed(consumer)
    }
}
