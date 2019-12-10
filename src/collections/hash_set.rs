//! This module contains the parallel iterator types for hash sets
//! (`HashSet<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::HashSet;
use std::hash::{BuildHasher, Hash};

use crate::iter::plumbing::*;
use crate::iter::*;

use crate::vec;

/// Parallel iterator over a hash set
#[derive(Debug)] // std doesn't Clone
pub struct IntoIter<T: Hash + Eq + Send> {
    inner: vec::IntoIter<T>,
}

into_par_vec! {
    HashSet<T, S> => IntoIter<T>,
    impl<T: Hash + Eq + Send, S: BuildHasher>
}

delegate_iterator! {
    IntoIter<T> => T,
    impl<T: Hash + Eq + Send>
}

/// Parallel iterator over an immutable reference to a hash set
#[derive(Debug)]
pub struct Iter<'a, T: Hash + Eq + Sync> {
    inner: vec::IntoIter<&'a T>,
}

impl<'a, T: Hash + Eq + Sync> Clone for Iter<'a, T> {
    fn clone(&self) -> Self {
        Iter {
            inner: self.inner.clone(),
        }
    }
}

into_par_vec! {
    &'a HashSet<T, S> => Iter<'a, T>,
    impl<'a, T: Hash + Eq + Sync, S: BuildHasher>
}

delegate_iterator! {
    Iter<'a, T> => &'a T,
    impl<'a, T: Hash + Eq + Sync + 'a>
}

// `HashSet` doesn't have a mutable `Iterator`
