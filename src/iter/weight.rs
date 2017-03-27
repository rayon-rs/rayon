use super::internal::*;
use super::*;

pub struct Weight<I> {
    base: I,
}

// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I>(base: I) -> Weight<I> {
    Weight { base: base }
}

impl<I> ParallelIterator for Weight<I>
    where I: ParallelIterator
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        self.base.drive_unindexed(consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}

impl<I: BoundedParallelIterator> BoundedParallelIterator for Weight<I> {
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        self.base.drive(consumer)
    }
}

impl<I: ExactParallelIterator> ExactParallelIterator for Weight<I> {
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<I: IndexedParallelIterator> IndexedParallelIterator for Weight<I> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        self.base.with_producer(callback)
    }
}
