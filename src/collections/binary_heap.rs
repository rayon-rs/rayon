//! This module contains the parallel iterator types for heaps
//! (`BinaryHeap<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::BinaryHeap;

use iter::*;
use iter::internal::*;

use vec;

impl<T: Ord + Send> IntoParallelIterator for BinaryHeap<T> {
    type Item = T;
    type Iter = IntoIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        IntoIter { inner: Vec::from(self).into_par_iter() }
    }
}

into_par_vec!{
    &'a BinaryHeap<T> => Iter<'a, T>,
    impl<'a, T: Ord + Sync>
}

// `BinaryHeap` doesn't have a mutable `Iterator`


delegate_indexed_iterator!{
    #[doc = "Parallel iterator over a binary heap"]
    IntoIter<T> => vec::IntoIter<T>,
    impl<T: Ord + Send>
}


delegate_indexed_iterator!{
    #[doc = "Parallel iterator over an immutable reference to a binary heap"]
    Iter<'a, T> => vec::IntoIter<&'a T>,
    impl<'a, T: Ord + Sync + 'a>
}
