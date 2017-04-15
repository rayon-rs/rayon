//! This module contains the parallel iterator types for results
//! (`Result<T, E>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use iter::*;
use iter::internal::*;
use std::sync::Mutex;

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


/// Collect an arbitrary `Result`-wrapped collection.
///
/// If any item is `Err`, then all previous `Ok` items collected are
/// discarded, and it returns that error.  If there are multiple errors, the
/// one returned is not deterministic.
impl<'a, C, T, E> FromParallelIterator<Result<T, E>> for Result<C, E>
    where C: FromParallelIterator<T>,
          T: Send,
          E: Send
{
    fn from_par_iter<I>(par_iter: I) -> Self
        where I: IntoParallelIterator<Item = Result<T, E>>
    {
        let saved_error = Mutex::new(None);
        let collection = par_iter
            .into_par_iter()
            .map(|item| match item {
                     Ok(item) => Some(item),
                     Err(error) => {
                         if let Ok(mut guard) = saved_error.lock() {
                             *guard = Some(error);
                         }
                         None
                     }
                 })
            .while_some()
            .collect();

        match saved_error.into_inner().unwrap() {
            Some(error) => Err(error),
            None => Ok(collection),
        }
    }
}
