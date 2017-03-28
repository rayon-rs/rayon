//! This module contains the parallel iterator types for options
//! (`Option<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use iter::*;
use iter::internal::*;
use std;

impl<T: Send> IntoParallelIterator for Option<T> {
    type Item = T;
    type Iter = IntoIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        IntoIter { opt: self }
    }
}

impl<'a, T: Sync> IntoParallelIterator for &'a Option<T> {
    type Item = &'a T;
    type Iter = Iter<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { inner: self.as_ref().into_par_iter() }
    }
}

impl<'a, T: Send> IntoParallelIterator for &'a mut Option<T> {
    type Item = &'a mut T;
    type Iter = IterMut<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        IterMut { inner: self.as_mut().into_par_iter() }
    }
}


/// Parallel iterator over an option
pub struct IntoIter<T: Send> {
    opt: Option<T>,
}

impl<T: Send> ParallelIterator for IntoIter<T> {
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<T: Send> BoundedParallelIterator for IntoIter<T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<T: Send> ExactParallelIterator for IntoIter<T> {
    fn len(&mut self) -> usize {
        match self.opt {
            Some(_) => 1,
            None => 0,
        }
    }
}

impl<T: Send> IndexedParallelIterator for IntoIter<T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(OptionProducer { opt: self.opt })
    }
}


delegate_indexed_iterator!{
    #[doc = "Parallel iterator over an immutable reference to an option"]
    Iter<'a, T> => IntoIter<&'a T>,
    impl<'a, T: Sync + 'a>
}


delegate_indexed_iterator!{
    #[doc = "Parallel iterator over a mutable reference to an option"]
    IterMut<'a, T> => IntoIter<&'a mut T>,
    impl<'a, T: Send + 'a>
}


/// Private producer for an option
struct OptionProducer<T: Send> {
    opt: Option<T>,
}

impl<T: Send> Producer for OptionProducer<T> {
    type Item = T;
    type IntoIter = std::option::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.opt.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let none = OptionProducer { opt: None };
        if index == 0 {
            (none, self)
        } else {
            (self, none)
        }
    }
}
