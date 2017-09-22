//! This module contains the parallel iterator types for B-Tree sets
//! (`BTreeSet<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::BTreeSet;

use iter::*;
use iter::internal::*;

use vec;

/// Parallel iterator over a B-Tree set
#[derive(Debug)] // std doesn't Clone
pub struct IntoIter<T: Ord + Send> {
    inner: vec::IntoIter<T>,
}

into_par_vec!{
    BTreeSet<T> => IntoIter<T>,
    impl<T: Ord + Send>
}

delegate_iterator!{
    IntoIter<T> => T,
    impl<T: Ord + Send>
}


/// Parallel iterator over an immutable reference to a B-Tree set
#[derive(Debug, Clone)]
pub struct Iter<'a, T: Ord + Sync + 'a> {
    inner: vec::IntoIter<&'a T>,
}

into_par_vec!{
    &'a BTreeSet<T> => Iter<'a, T>,
    impl<'a, T: Ord + Sync>
}

delegate_iterator!{
    Iter<'a, T> => &'a T,
    impl<'a, T: Ord + Sync + 'a>
}


// `BTreeSet` doesn't have a mutable `Iterator`
