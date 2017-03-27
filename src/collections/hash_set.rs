//! This module contains the parallel iterator types for hash sets
//! (`HashSet<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::HashSet;
use std::hash::{Hash, BuildHasher};

use iter::*;
use iter::internal::*;

use vec;

into_par_vec!{
    HashSet<T, S> => IntoIter<T>,
    impl<T: Hash + Eq + Send, S: BuildHasher>
}

into_par_vec!{
    &'a HashSet<T, S> => Iter<'a, T>,
    impl<'a, T: Hash + Eq + Sync, S: BuildHasher>
}

// `HashSet` doesn't have a mutable `Iterator`


delegate_iterator!{
    #[doc = "Parallel iterator over a hash set"]
    IntoIter<T> => vec::IntoIter<T>,
    impl<T: Hash + Eq + Send>
}


delegate_iterator!{
    #[doc = "Parallel iterator over an immutable reference to a hash set"]
    Iter<'a, T> => vec::IntoIter<&'a T>,
    impl<'a, T: Hash + Eq + Sync + 'a>
}
