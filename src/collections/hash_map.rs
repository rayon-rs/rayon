//! This module contains the parallel iterator types for hash maps
//! (`HashMap<K, V>`). You will rarely need to interact with it directly
//! unless you have need to name one of the iterator types.

use std::collections::HashMap;
use std::hash::{Hash, BuildHasher};

use iter::*;
use iter::internal::*;

use vec;

into_par_vec!{
    HashMap<K, V, S> => IntoIter<K, V>,
    impl<K: Hash + Eq + Send, V: Send, S: BuildHasher>
}

into_par_vec!{
    &'a HashMap<K, V, S> => Iter<'a, K, V>,
    impl<'a, K: Hash + Eq + Sync, V: Sync, S: BuildHasher>
}

into_par_vec!{
    &'a mut HashMap<K, V, S> => IterMut<'a, K, V>,
    impl<'a, K: Hash + Eq + Sync, V: Send, S: BuildHasher>
}


delegate_iterator!{
    #[doc = "Parallel iterator over a hash map"]
    IntoIter<K, V> => vec::IntoIter<(K, V)>,
    impl<K: Hash + Eq + Send, V: Send>
}


delegate_iterator!{
    #[doc = "Parallel iterator over an immutable reference to a hash map"]
    Iter<'a, K, V> => vec::IntoIter<(&'a K, &'a V)>,
    impl<'a, K: Hash + Eq + Sync + 'a, V: Sync + 'a>
}


delegate_iterator!{
    #[doc = "Parallel iterator over a mutable reference to a hash map"]
    IterMut<'a, K, V> => vec::IntoIter<(&'a K, &'a mut V)>,
    impl<'a, K: Hash + Eq + Sync + 'a, V: Send + 'a>
}
