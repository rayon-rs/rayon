//! This module contains the parallel iterator types for double-ended queues
//! (`VecDeque<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::VecDeque;

use iter::*;
use iter::internal::*;

use slice;
use vec;

into_par_vec!{
    VecDeque<T> => IntoIter<T>,
    impl<T: Send>
}

impl<'a, T: Sync> IntoParallelIterator for &'a VecDeque<T> {
    type Item = &'a T;
    type Iter = Iter<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        let (a, b) = self.as_slices();
        Iter { inner: a.into_par_iter().chain(b) }
    }
}

impl<'a, T: Send> IntoParallelIterator for &'a mut VecDeque<T> {
    type Item = &'a mut T;
    type Iter = IterMut<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        let (a, b) = self.as_mut_slices();
        IterMut { inner: a.into_par_iter().chain(b) }
    }
}


delegate_indexed_iterator!{
    #[doc = "Parallel iterator over a double-ended queue"]
    IntoIter<T> => vec::IntoIter<T>,
    impl<T: Send>
}


delegate_indexed_iterator_item!{
    #[doc = "Parallel iterator over an immutable reference to a double-ended queue"]
    Iter<'a, T> => Chain<slice::Iter<'a, T>, slice::Iter<'a, T>> : &'a T,
    impl<'a, T: Sync + 'a>
}


delegate_indexed_iterator_item!{
    #[doc = "Parallel iterator over a mutable reference to a double-ended queue"]
    IterMut<'a, T> => Chain<slice::IterMut<'a, T>, slice::IterMut<'a, T>> : &'a mut T,
    impl<'a, T: Send + 'a>
}
