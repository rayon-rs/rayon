//! This module contains the parallel iterator types for results
//! (`Result<T, E>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use iter::*;
use iter::internal::*;

use option;

impl<T: Send, E> IntoParallelIterator for Result<T, E> {
    type Item = T;
    type Iter = IntoIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        IntoIter { inner: self.ok().into_par_iter() }
    }
}

impl<'a, T: Sync, E> IntoParallelIterator for &'a Result<T, E> {
    type Item = &'a T;
    type Iter = Iter<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { inner: self.as_ref().ok().into_par_iter() }
    }
}

impl<'a, T: Send, E> IntoParallelIterator for &'a mut Result<T, E> {
    type Item = &'a mut T;
    type Iter = IterMut<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        IterMut { inner: self.as_mut().ok().into_par_iter() }
    }
}


delegate_indexed_iterator!{
    #[doc = "Parallel iterator over a result"]
    IntoIter<T> => option::IntoIter<T>,
    impl<T: Send>
}


delegate_indexed_iterator!{
    #[doc = "Parallel iterator over an immutable reference to a result"]
    Iter<'a, T> => option::IntoIter<&'a T>,
    impl<'a, T: Sync + 'a>
}


delegate_indexed_iterator!{
    #[doc = "Parallel iterator over a mutable reference to a result"]
    IterMut<'a, T> => option::IntoIter<&'a mut T>,
    impl<'a, T: Send + 'a>
}
