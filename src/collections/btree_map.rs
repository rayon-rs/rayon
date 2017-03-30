//! This module contains the parallel iterator types for B-Tree maps
//! (`BTreeMap<K, V>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::BTreeMap;

use iter::*;
use iter::internal::*;

use vec;

into_par_vec!{
    BTreeMap<K, V> => IntoIter<K, V>,
    impl<K: Ord + Send, V: Send>
}

into_par_vec!{
    &'a BTreeMap<K, V> => Iter<'a, K, V>,
    impl<'a, K: Ord + Sync, V: Sync>
}

into_par_vec!{
    &'a mut BTreeMap<K, V> => IterMut<'a, K, V>,
    impl<'a, K: Ord + Sync, V: Send>
}


delegate_iterator!{
    #[doc = "Parallel iterator over a B-Tree map"]
    IntoIter<K, V> => vec::IntoIter<(K, V)>,
    impl<K: Ord + Send, V: Send>
}


delegate_iterator!{
    #[doc = "Parallel iterator over an immutable reference to a B-Tree map"]
    Iter<'a, K, V> => vec::IntoIter<(&'a K, &'a V)>,
    impl<'a, K: Ord + Sync + 'a, V: Sync + 'a>
}


delegate_iterator!{
    #[doc = "Parallel iterator over a mutable reference to a B-Tree map"]
    IterMut<'a, K, V> => vec::IntoIter<(&'a K, &'a mut V)>,
    impl<'a, K: Ord + Sync + 'a, V: Send + 'a>
}
