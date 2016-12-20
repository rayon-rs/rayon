use super::internal::*;
use super::*;
use std;

pub struct OptionIter<T: Send> {
    opt: Option<T>,
}

impl<T: Send> IntoParallelIterator for Option<T> {
    type Item = T;
    type Iter = OptionIter<Self::Item>;

    fn into_par_iter(self) -> Self::Iter {
        OptionIter { opt: self }
    }
}

impl<T: Send, E> IntoParallelIterator for Result<T, E> {
    type Item = T;
    type Iter = OptionIter<Self::Item>;

    fn into_par_iter(self) -> Self::Iter {
        self.ok().into_par_iter()
    }
}

impl<'a, T: Sync> IntoParallelIterator for &'a Option<T> {
    type Item = &'a T;
    type Iter = OptionIter<Self::Item>;

    fn into_par_iter(self) -> Self::Iter {
        self.as_ref().into_par_iter()
    }
}

impl<'a, T: Sync, E> IntoParallelIterator for &'a Result<T, E> {
    type Item = &'a T;
    type Iter = OptionIter<Self::Item>;

    fn into_par_iter(self) -> Self::Iter {
        self.as_ref().ok().into_par_iter()
    }
}

impl<'a, T: Send> IntoParallelIterator for &'a mut Option<T> {
    type Item = &'a mut T;
    type Iter = OptionIter<Self::Item>;

    fn into_par_iter(self) -> Self::Iter {
        self.as_mut().into_par_iter()
    }
}

impl<'a, T: Send, E> IntoParallelIterator for &'a mut Result<T, E> {
    type Item = &'a mut T;
    type Iter = OptionIter<Self::Item>;

    fn into_par_iter(self) -> Self::Iter {
        self.as_mut().ok().into_par_iter()
    }
}

/// ////////////////////////////////////////////////////////////////////////

impl<T: Send> ParallelIterator for OptionIter<T> {
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

impl<T: Send> BoundedParallelIterator for OptionIter<T> {
    fn upper_bound(&mut self) -> usize {
        ExactParallelIterator::len(self)
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<T: Send> ExactParallelIterator for OptionIter<T> {
    fn len(&mut self) -> usize {
        match self.opt {
            Some(_) => 1,
            None => 0,
        }
    }
}

impl<T: Send> IndexedParallelIterator for OptionIter<T> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(OptionProducer { opt: self.opt })
    }
}

/// ////////////////////////////////////////////////////////////////////////

pub struct OptionProducer<T: Send> {
    opt: Option<T>,
}

impl<T: Send> Producer for OptionProducer<T> {
    fn cost(&mut self, len: usize) -> f64 {
        len as f64
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

impl<T: Send> IntoIterator for OptionProducer<T> {
    type Item = T;
    type IntoIter = std::option::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.opt.into_iter()
    }
}
