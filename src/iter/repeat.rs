use super::*;
use super::internal::*;
use std::usize;

/// Iterator adapter for [the `repeat()` function](fn.split.html).
pub struct Repeat<T: Clone + Send> {
    element: T,
}

/// Creates a parallel iterator that endlessly repeats `elt` (by
/// cloning it). Note that this iterator has "infinite" length, so
/// typically you would want to use `zip` or `take` or some other
/// means to shorten it.
///
/// Example:
///
/// ```
/// use rayon::prelude::*;
/// use rayon::iter::repeat;
/// let x: Vec<(i32, i32)> = repeat(22).zip(0..3).collect();
/// assert_eq!(x, vec![(22, 0), (22, 1), (22, 2)]);
/// ```
pub fn repeat<T: Clone + Send>(elt: T) -> Repeat<T> {
    Repeat { element: elt }
}

impl<T> ParallelIterator for Repeat<T>
    where T: Clone + Send
{
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge_unindexed(self, consumer)
    }
}

impl<T> IndexedParallelIterator for Repeat<T>
    where T: Clone + Send
{
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(RepeatProducer { repeat: self.element })
    }

    fn len(&mut self) -> usize {
        usize::MAX
    }
}


/// Producer

struct RepeatProducer<T: Clone + Send> {
    repeat: T,
}

impl<T: Clone + Send> Producer for RepeatProducer<T> {
    type Item = T;
    type IntoIter = RepeatIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        RepeatIter { repeat: self.repeat }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        (RepeatProducer { repeat: self.repeat.clone() }, RepeatProducer { repeat: self.repeat })
    }
}

impl<T: Clone + Send> UnindexedProducer for Repeat<T> {
    type Item = T;

    fn split(self) -> (Self, Option<Self>) {
        (Repeat { element: self.element.clone() }, Some(Repeat { element: self.element }))
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<T>
    {
        folder.consume_iter(RepeatIter { repeat: self.element })
    }
}

/// Repeat Iter Create As Repeat Does Not Have ExactSizeIterator

struct RepeatIter<T> {
    repeat: T,
}

impl<T: Clone> Iterator for RepeatIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        Some(self.repeat.clone())
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

impl<A: Clone> DoubleEndedIterator for RepeatIter<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        Some(self.repeat.clone())
    }
}

impl<T: Clone> ExactSizeIterator for RepeatIter<T> {
    fn len(&self) -> usize {
        // FIXME this is .. sort of wrong. After all, we produce
        // *more* than `usize::MAX`, potentially. Is there a better
        // alternative?
        usize::MAX
    }
}
