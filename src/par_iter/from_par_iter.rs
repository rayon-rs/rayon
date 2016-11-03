use super::{ParallelIterator, ExactParallelIterator};

use std::collections::{HashSet, HashMap};
use std::collections::{BTreeSet, BTreeMap};
use std::hash::{BuildHasher, Hash};
use std::collections::LinkedList;

pub trait FromParallelIterator<PAR_ITER> {
    fn from_par_iter(par_iter: PAR_ITER) -> Self;
}

/// Collect items from a parallel iterator into a freshly allocated
/// vector. This is very efficient, but requires precise knowledge of
/// the number of items being iterated.
impl<PAR_ITER, T> FromParallelIterator<PAR_ITER> for Vec<T>
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
impl<PAR_ITER, T> FromParallelIterator<PAR_ITER> for LinkedList<T>
    where PAR_ITER: ParallelIterator<Item=T>,
          T: Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        par_iter.map(|elem| { let mut list = LinkedList::new(); list.push_back(elem); list })
                .reduce_with(|mut list1, mut list2| { list1.append(&mut list2); list1 })
                .unwrap_or_else(|| LinkedList::new())
    }
}

/// Collect items from a parallel iterator into a hashmap.
///
/// WARNING: If multiple keys map to the same key, it is not defined
/// which one will win as the final value.
impl<PAR_ITER, K, V, S> FromParallelIterator<PAR_ITER> for HashMap<K, V, S>
    where PAR_ITER: ParallelIterator<Item=(K, V)> + Send,
          K: Eq + Hash + Send,
          V: Send,
          S: BuildHasher + Default + Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        let mut map = HashMap::default();
        par_iter.for_each_atomic(|(key, value)| {
            map.insert(key, value);
        });
        map
    }
}

/// Collect items from a parallel iterator into a btreemap.
///
/// WARNING: If multiple keys map to the same key, it is not defined
/// which one will win as the final value.
impl<PAR_ITER, K, V> FromParallelIterator<PAR_ITER> for BTreeMap<K, V>
    where PAR_ITER: ParallelIterator<Item=(K, V)> + Send,
          K: Ord + Send,
          V: Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        let mut map = BTreeMap::default();
        par_iter.for_each_atomic(|(key, value)| {
            map.insert(key, value);
        });
        map
    }
}

/// Collect items from a parallel iterator into a hashset.
impl<PAR_ITER, K, S> FromParallelIterator<PAR_ITER> for HashSet<K, S>
    where PAR_ITER: ParallelIterator<Item=K> + Send,
          K: Eq + Hash + Send,
          S: BuildHasher + Default + Send,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        let mut map = HashSet::default();
        par_iter.for_each_atomic(|key| {
            map.insert(key);
        });
        map
    }
}

/// Collect items from a parallel iterator into a btreeset.
impl<PAR_ITER, K> FromParallelIterator<PAR_ITER> for BTreeSet<K>
    where PAR_ITER: ParallelIterator<Item=K> + Send,
          K: Send + Ord,
{
    fn from_par_iter(par_iter: PAR_ITER) -> Self {
        let mut map = BTreeSet::default();
        par_iter.for_each_atomic(|key| {
            map.insert(key);
        });
        map
    }
}
