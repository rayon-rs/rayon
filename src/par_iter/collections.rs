use super::{IntoParallelIterator, ParallelIterator};
use super::chain::ChainIter;
use super::slice::SliceIter;
use super::slice_mut::SliceIterMut;
use super::vec::VecIter;
use std::collections::{BinaryHeap, BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque};
use std::iter::FromIterator;

// This could almost be replaced with a blanket implementation for *all*
// `IntoIterator` types, but we'd need to specialize a few like `Vec`.
macro_rules! vectorized {
    (@impl_body) => {
        type Item = <Self as IntoIterator>::Item;
        type Iter = VecIter<Self::Item>;

        fn into_par_iter(self) -> Self::Iter {
            Vec::from_iter(self).into_par_iter()
        }
    };
    ( impl<'a, $($c:ident),*> IntoParallelIterator for &'a mut $t:ty  ) => {
        impl<'a, $($c),*> IntoParallelIterator for &'a mut $t
            where &'a mut $t: IntoIterator,
                  <&'a mut $t as IntoIterator>::Item: Send
        {
            vectorized!{@impl_body}
        }
    };
    ( impl<'a, $($c:ident),*> IntoParallelIterator for &'a $t:ty  ) => {
        impl<'a, $($c),*> IntoParallelIterator for &'a $t
            where &'a $t: IntoIterator,
                  <&'a $t as IntoIterator>::Item: Send
        {
            vectorized!{@impl_body}
        }
    };
    ( impl<$($c:ident),*> IntoParallelIterator for $t:ty  ) => {
        impl<$($c),*> IntoParallelIterator for $t
            where $t: IntoIterator,
                  <$t as IntoIterator>::Item: Send
        {
            vectorized!{@impl_body}
        }
    };
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

impl<'a, T: Sync> IntoParallelIterator for &'a VecDeque<T> {
    type Item = &'a T;
    type Iter = ChainIter<SliceIter<'a, T>, SliceIter<'a, T>>;

    fn into_par_iter(self) -> Self::Iter {
        let (a, b) = self.as_slices();
        a.into_par_iter().chain(b)
    }
}

impl<'a, T: Send> IntoParallelIterator for &'a mut VecDeque<T> {
    type Item = &'a mut T;
    type Iter = ChainIter<SliceIterMut<'a, T>, SliceIterMut<'a, T>>;

    fn into_par_iter(self) -> Self::Iter {
        let (a, b) = self.as_mut_slices();
        a.into_par_iter().chain(b)
    }
}
