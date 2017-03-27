//! This module contains the parallel iterator types for B-Tree sets
//! (`BTreeSet<T>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::BTreeSet;

use iter::*;
use iter::internal::*;

use vec;

into_par_vec!{
    BTreeSet<T> => IntoIter<T>,
    impl<T: Ord + Send>
}

into_par_vec!{
    &'a BTreeSet<T> => Iter<'a, T>,
    impl<'a, T: Ord + Sync>
}

// `BTreeSet` doesn't have a mutable `Iterator`


delegate_iterator!{
    #[doc = "Parallel iterator over a B-Tree set"]
    IntoIter<T> => vec::IntoIter<T>,
    impl<T: Ord + Send>
}


delegate_iterator!{
    #[doc = "Parallel iterator over an immutable reference to a B-Tree set"]
    Iter<'a, T> => vec::IntoIter<&'a T>,
    impl<'a, T: Ord + Sync + 'a>
}
