use super::{ParallelIterator, ExactParallelIterator};

use collections::vec_tree::VecTree;
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

/// Collect items from a parallel iterator into a freshly allocated
/// vector. This is very efficient, but requires precise knowledge of
/// the number of items being iterated.
impl<PAR_ITER, T> FromParIter<PAR_ITER> for VecTree<T>
    where PAR_ITER: ParallelIterator<Item=T>,
          T: Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        par_iter.fold(|| Vec::new(), |mut vec, elem| { vec.push(elem); vec })
                .map(VecTree::leaf)
                .reduce_with(VecTree::branch)
                .unwrap_or_else(|| VecTree::new())
    }
}

/// Collect items from a parallel iterator into a hashmap. To some
/// extent, a proof of concept, since it requires an intermediate
/// allocation into a vector at the moment.
impl<PAR_ITER, K: Eq + Hash, V> FromParIter<PAR_ITER> for HashMap<K, V>
    where PAR_ITER: ParallelIterator<Item=(K, V)>,
          (K, V): Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        let vec_tree: VecTree<(K, V)> = par_iter.collect();
        let mut map = HashMap::with_capacity(vec_tree.len());
        vec_tree.foreach(&mut |(key, value)| {
            let _ = map.insert(key, value);
        });
        map
    }
}
