use super::{ParallelIterator, ExactParallelIterator};

use std::collections::HashMap;
use std::hash::Hash;
use std::collections::LinkedList;

pub trait FromParIter<PAR_ITER> {
    fn from_par_iter(par_iter: PAR_ITER) -> Self;
}

/// Collect items from a parallel iterator into a freshly allocated
/// vector. This is very efficient, but requires precise knowledge of
/// the number of items being iterated.
impl<PAR_ITER, T> FromParIter<PAR_ITER> for Vec<T>
    where PAR_ITER: ExactParallelIterator<Item=T>,
          T: Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        let mut vec = vec![];
        par_iter.collect_into(&mut vec);
        vec
    }
}

/// Collect items from a parallel iterator into a freshly allocated
/// linked list.
impl<PAR_ITER, T> FromParIter<PAR_ITER> for LinkedList<T>
    where PAR_ITER: ParallelIterator<Item=T>,
          T: Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        par_iter.map(|elem| { let mut list = LinkedList::new(); list.push_back(elem); list })
                .reduce_with(|mut list1, mut list2| { list1.append(&mut list2); list1 })
                .unwrap_or_else(|| LinkedList::new())
    }
}

/// Collect items from a parallel iterator into a hashmap. To some
/// extent, a proof of concept, since it requires an intermediate
/// allocation into a vector at the moment.
impl<PAR_ITER, K: Eq + Hash, V> FromParIter<PAR_ITER> for HashMap<K, V>
    where PAR_ITER: ExactParallelIterator<Item=(K, V)>,
          (K, V): Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        let vec: Vec<(K, V)> = par_iter.collect();
        vec.into_iter().collect()
    }
}
