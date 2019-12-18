//! This module contains the parallel iterator types for double-ended queues
//! (`VecDeque<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::VecDeque;

use crate::iter::plumbing::*;
use crate::iter::*;

use crate::slice;
use crate::vec;

/// Parallel iterator over a double-ended queue
#[derive(Debug, Clone)]
pub struct IntoIter<T: Send> {
    inner: vec::IntoIter<T>,
}

into_par_vec! {
    VecDeque<T> => IntoIter<T>,
    impl<T: Send>
}

delegate_indexed_iterator! {
    IntoIter<T> => T,
    impl<T: Send>
}

/// Parallel iterator over an immutable reference to a double-ended queue
#[derive(Debug)]
pub struct Iter<'a, T: Sync> {
    inner: Chain<slice::Iter<'a, T>, slice::Iter<'a, T>>,
}

impl<'a, T: Sync> Clone for Iter<'a, T> {
    fn clone(&self) -> Self {
        Iter {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, T: Sync> IntoParallelIterator for &'a VecDeque<T> {
    type Item = &'a T;
    type Iter = Iter<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        let (a, b) = self.as_slices();
        Iter {
            inner: a.into_par_iter().chain(b),
        }
    }
}

delegate_indexed_iterator! {
    Iter<'a, T> => &'a T,
    impl<'a, T: Sync + 'a>
}

/// Parallel iterator over a mutable reference to a double-ended queue
#[derive(Debug)]
pub struct IterMut<'a, T: Send> {
    inner: Chain<slice::IterMut<'a, T>, slice::IterMut<'a, T>>,
}

impl<'a, T: Send> IntoParallelIterator for &'a mut VecDeque<T> {
    type Item = &'a mut T;
    type Iter = IterMut<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        let (a, b) = self.as_mut_slices();
        IterMut {
            inner: a.into_par_iter().chain(b),
        }
    }
}

delegate_indexed_iterator! {
    IterMut<'a, T> => &'a mut T,
    impl<'a, T: Send + 'a>
}
