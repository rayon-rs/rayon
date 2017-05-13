use super::internal::*;
use super::*;
use iter::*;
use iter::internal::*;
use std;
use std::iter;
use std::usize;


pub struct Repeat<T> {
    element: T,
}

pub fn repeat<T: Clone>(elt: T) -> Repeat<T> {
    Repeat{element: elt}
}

impl<T: Clone> Clone for Repeat<T>{
    fn clone(&self) -> Repeat<T> {
        Repeat{ element: self.element.clone()}
    }
}

impl<T> ParallelIterator for Repeat<T>
    where T: Clone + Send
{
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
       where C: UnindexedConsumer<Self::Item>
   {
       bridge(self, consumer)
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

    fn with_producer<CB>(self, callback: CB) -> CB :: Output
        where CB: ProducerCallback<Self::Item>
        {
            callback.callback(RepeatProducer{ repeat: self})
        }

    fn len(&mut self) -> usize { usize::MAX }
}


/// Producer

struct RepeatProducer<P> {
    repeat: Repeat<P>,
}

impl<T: Clone + Send> Producer for RepeatProducer<T> {
    type Item = T;
    type IntoIter = RepeatIter<T>;

    fn into_iter(self) -> Self::IntoIter{
        RepeatIter{repeat: self.repeat}
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        (
            RepeatProducer{repeat: self.repeat.clone()},
            RepeatProducer{repeat: self.repeat.clone()}
        )
    }
}

impl<T: Clone + Send> UnindexedProducer for Repeat<T> {
    type Item = T;

    fn split(self) -> (Self, Option<Self>) {
        (Repeat{element: self.element.clone()}, Some(Repeat{element: self.element.clone()}))
    }

    fn fold_with<F>(self, folder: F) -> F where F: Folder<T> {
        folder.consume_iter(RepeatIter{repeat: Repeat{element: self.element}})
    }
}

/// Repeat Iter Create As Repeat Does Not Have ExactSizeIterator

struct RepeatIter<T> {
    repeat: Repeat<T>,
}

impl<T: Clone> Iterator for RepeatIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> { Some(self.repeat.element.clone()) }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) { (usize::MAX, None) }
}

impl<A: Clone> DoubleEndedIterator for RepeatIter<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A> { Some(self.repeat.element.clone()) }
}

impl<T: Clone> ExactSizeIterator for RepeatIter<T> {
    fn len(&self) -> usize {
        usize::MAX
    }
}
