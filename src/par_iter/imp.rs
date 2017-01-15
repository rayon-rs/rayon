use super::*;

/// The `ParallelIterator` implementation interface.
pub trait ParallelIteratorImpl: Sized {
    type Item: Send;

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    fn drive_unindexed<C>(self, consumer: C) -> C::Result where C: UnindexedConsumer<Self::Item>;

    /// Returns the number of items produced by this iterator, if known
    /// statically. This can be used by consumers to trigger special fast
    /// paths. Therefore, if `Some(_)` is returned, this iterator must only
    /// use the (indexed) `Consumer` methods when driving a consumer, such
    /// as `split_at()`. Calling `UnindexedConsumer::split_off_left()` or
    /// other `UnindexedConsumer` methods -- or returning an inaccurate
    /// value -- may result in panics.
    ///
    /// This is hidden & considered internal for now, until we decide
    /// whether it makes sense for a public API.  Right now it is only used
    /// to optimize `collect`  for want of true Rust specialization.
    #[doc(hidden)]
    fn opt_len(&mut self) -> Option<usize> {
        None
    }
}

impl<T> ParallelIterator for T where T: ParallelIteratorImpl {}


/// A trait for parallel iterators items where the precise number of
/// items is not known, but we can at least give an upper-bound. These
/// sorts of iterators result from filtering.
pub trait BoundedParallelIteratorImpl: ParallelIterator {
    fn impl_upper_bound(&mut self) -> usize;

    /// Internal method used to define the behavior of this parallel
    /// iterator. You should not need to call this directly.
    fn drive<'c, C: Consumer<Self::Item>>(self, consumer: C) -> C::Result;
}

impl<T> BoundedParallelIterator for T
    where T: BoundedParallelIteratorImpl
{
    fn upper_bound(&mut self) -> usize {
        self.impl_upper_bound()
    }
}


/// A trait for parallel iterators items where the precise number of
/// items is known. This occurs when e.g. iterating over a
/// vector. Knowing precisely how many items will be produced is very
/// useful.
pub trait ExactParallelIteratorImpl: BoundedParallelIterator {
    /// Produces an exact count of how many items this iterator will
    /// produce, presuming no panic occurs.
    fn impl_len(&mut self) -> usize;
}

impl<T> ExactParallelIterator for T
    where T: ExactParallelIteratorImpl
{
    fn len(&mut self) -> usize {
        self.impl_len()
    }
}


/// An iterator that supports "random access" to its data, meaning
/// that you can split it at arbitrary indices and draw data from
/// those points.
pub trait IndexedParallelIteratorImpl: ExactParallelIterator {
    /// Convert this parallel iterator into a producer that can be
    /// used to request the items.
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output;
}

impl<T> IndexedParallelIterator for T where T: IndexedParallelIteratorImpl {}
