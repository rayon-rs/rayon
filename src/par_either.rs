use either::Either::{self, Left, Right};

use iter::*;
use iter::internal::*;

/// `Either<L, R>` is a parallel iterator if both `L` and `R` are parallel iterators.
impl<L, R> ParallelIterator for Either<L, R>
    where L: ParallelIterator,
          R: ParallelIterator<Item = L::Item>
{
    type Item = L::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        match self {
            Left(iter) => iter.drive_unindexed(consumer),
            Right(iter) => iter.drive_unindexed(consumer),
        }
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.as_mut().either(L::opt_len, R::opt_len)
    }
}

impl<L, R> IndexedParallelIterator for Either<L, R>
    where L: IndexedParallelIterator,
          R: IndexedParallelIterator<Item = L::Item>
{
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        match self {
            Left(iter) => iter.drive(consumer),
            Right(iter) => iter.drive(consumer),
        }
    }

    fn len(&mut self) -> usize {
        self.as_mut().either(L::len, R::len)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        match self {
            Left(iter) => iter.with_producer(callback),
            Right(iter) => iter.with_producer(callback),
        }
    }
}
