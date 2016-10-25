use super::ExactParallelIterator;

use std::collections::HashMap;
use std::hash::Hash;

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
