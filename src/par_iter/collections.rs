use super::IntoParallelIterator;
use super::vec::VecIter;
use std::collections::{BinaryHeap, BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque};
use std::iter::FromIterator;

// This could almost be replaced with a blanket implementation for *all*
// `IntoIterator` types, but we'd need specialization a few like `Vec`.
macro_rules! vectorized {
    ( impl<$($c:tt),*> IntoParallelIterator for $t:ty ) => {
        impl<$($c),*> IntoParallelIterator for $t
            where $t: IntoIterator,
                  <$t as IntoIterator>::Item: Send
        {
            type Item = <Self as IntoIterator>::Item;
            type Iter = VecIter<Self::Item>;

            fn into_par_iter(self) -> Self::Iter {
                Vec::from_iter(self).into_par_iter()
            }
        }
    }
}


impl<T: Send> IntoParallelIterator for BinaryHeap<T> {
    type Item = T;
    type Iter = VecIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        Vec::from(self).into_par_iter()
    }
}

vectorized!{ impl<'a, T> IntoParallelIterator for &'a BinaryHeap<T> }
// `BinaryHeap` doesn't have a mutable `Iterator`


vectorized!{ impl<K, V> IntoParallelIterator for BTreeMap<K, V> }
vectorized!{ impl<'a, K, V> IntoParallelIterator for &'a BTreeMap<K, V> }
vectorized!{ impl<'a, K, V> IntoParallelIterator for &'a mut BTreeMap<K, V> }


vectorized!{ impl<T> IntoParallelIterator for BTreeSet<T> }
vectorized!{ impl<'a, T> IntoParallelIterator for &'a BTreeSet<T> }
// `BTreeSet` doesn't have a mutable `Iterator`


vectorized!{ impl<K, V, S> IntoParallelIterator for HashMap<K, V, S> }
vectorized!{ impl<'a, K, V, S> IntoParallelIterator for &'a HashMap<K, V, S> }
vectorized!{ impl<'a, K, V, S> IntoParallelIterator for &'a mut HashMap<K, V, S> }


vectorized!{ impl<T, S> IntoParallelIterator for HashSet<T, S> }
vectorized!{ impl<'a, T, S> IntoParallelIterator for &'a HashSet<T, S> }
// `HashSet` doesn't have a mutable `Iterator`


vectorized!{ impl<T> IntoParallelIterator for LinkedList<T> }
vectorized!{ impl<'a, T> IntoParallelIterator for &'a LinkedList<T> }
vectorized!{ impl<'a, T> IntoParallelIterator for &'a mut LinkedList<T> }


vectorized!{ impl<T> IntoParallelIterator for VecDeque<T> }
vectorized!{ impl<'a, T> IntoParallelIterator for &'a VecDeque<T> }
vectorized!{ impl<'a, T> IntoParallelIterator for &'a mut VecDeque<T> }
// TODO - we could easily write these last two directly, no temp vector,
// with `as_slices`/`as_mut_slices` and a `chain` combinator.
