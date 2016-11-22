use super::{IntoParallelIterator, ParallelIterator};

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::collections::LinkedList;
use std::collections::{BinaryHeap, VecDeque};

pub trait FromParallelIterator<ITEM>
    where ITEM: Send
{
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = ITEM>;
}


fn combine<PAR_ITER, START, COLL>(par_iter: PAR_ITER, make_start: START) -> COLL
    where PAR_ITER: IntoParallelIterator,
          START: FnOnce(&LinkedList<Vec<PAR_ITER::Item>>) -> COLL,
          COLL: Extend<PAR_ITER::Item>
{
    let list = par_iter.into_par_iter()
        .fold(Vec::new, |mut vec, elem| {
            vec.push(elem);
            vec
        })
        .collect();

    let start = make_start(&list);
    list.into_iter()
        .fold(start, |mut coll, vec| {
            coll.extend(vec);
            coll
        })
}

fn combined_len<T>(list: &LinkedList<Vec<T>>) -> usize {
    list.iter().map(Vec::len).sum()
}


// See the `collect` module for the `Vec<T>` implementation.

/// Collect items from a parallel iterator into a vecdeque.
impl<T> FromParallelIterator<T> for VecDeque<T>
    where T: Send
{
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = T>
    {
        Vec::from_par_iter(par_iter).into()
    }
}

/// Collect items from a parallel iterator into a binaryheap.
/// The heap-ordering is calculated serially after all items are collected.
impl<T> FromParallelIterator<T> for BinaryHeap<T>
    where T: Ord + Send
{
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = T>
    {
        Vec::from_par_iter(par_iter).into()
    }
}

/// Collect items from a parallel iterator into a freshly allocated
/// linked list.
impl<T> FromParallelIterator<T> for LinkedList<T>
    where T: Send
{
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = T>
    {
        par_iter.into_par_iter()
            .map(|elem| {
                let mut list = LinkedList::new();
                list.push_back(elem);
                list
            })
            .reduce_with(|mut list1, mut list2| {
                list1.append(&mut list2);
                list1
            })
            .unwrap_or_else(LinkedList::new)
    }
}

/// Collect (key, value) pairs from a parallel iterator into a
/// hashmap. If multiple pairs correspond to the same key, then the
/// ones produced earlier in the parallel iterator will be
/// overwritten, just as with a sequential iterator.
impl<K, V, S> FromParallelIterator<(K, V)> for HashMap<K, V, S>
    where K: Eq + Hash + Send,
          V: Send,
          S: BuildHasher + Default + Send
{
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = (K, V)>
    {
        // See the map_collect benchmarks in rayon-demo for different strategies.
        combine(par_iter, |list| {
            let len = combined_len(list);
            HashMap::with_capacity_and_hasher(len, Default::default())
        })
    }
}

/// Collect (key, value) pairs from a parallel iterator into a
/// btreemap. If multiple pairs correspond to the same key, then the
/// ones produced earlier in the parallel iterator will be
/// overwritten, just as with a sequential iterator.
impl<K, V> FromParallelIterator<(K, V)> for BTreeMap<K, V>
    where K: Ord + Send,
          V: Send
{
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = (K, V)>
    {
        combine(par_iter, |_| BTreeMap::new())
    }
}

/// Collect values from a parallel iterator into a hashset.
impl<V, S> FromParallelIterator<V> for HashSet<V, S>
    where V: Eq + Hash + Send,
          S: BuildHasher + Default + Send
{
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = V>
    {
        combine(par_iter, |list| {
            let len = combined_len(list);
            HashSet::with_capacity_and_hasher(len, Default::default())
        })
    }
}

/// Collect values from a parallel iterator into a btreeset.
impl<V> FromParallelIterator<V> for BTreeSet<V>
    where V: Send + Ord
{
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = V>
    {
        combine(par_iter, |_| BTreeSet::new())
    }
}

/// Collect characters from a parallel iterator into a string.
impl FromParallelIterator<char> for String {
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = char>
    {
        // This is like `combine`, but `Vec<char>` is less efficient to deal
        // with than `String`, so instead collect to `LinkedList<String>`.
        let list: LinkedList<_> = par_iter.into_par_iter()
            .fold(String::new, |mut string, ch| {
                string.push(ch);
                string
            })
            .collect();

        let len = list.iter().map(String::len).sum();
        let start = String::with_capacity(len);
        list.into_iter()
            .fold(start, |mut string, sub| {
                string.push_str(&sub);
                string
            })
    }
}

/// Collect string slices from a parallel iterator into a string.
impl<'a> FromParallelIterator<&'a str> for String {
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = &'a str>
    {
        combine(par_iter, |list| {
            let len = list.iter()
                .map(|vec| -> usize { vec.iter().cloned().map(str::len).sum() })
                .sum();
            String::with_capacity(len)
        })
    }
}

/// Collect strings from a parallel iterator into one large string.
impl FromParallelIterator<String> for String {
    fn from_par_iter<PAR_ITER>(par_iter: PAR_ITER) -> Self
        where PAR_ITER: IntoParallelIterator<Item = String>
    {
        combine(par_iter, |list| {
            let len = list.iter()
                .map(|vec| -> usize { vec.iter().map(String::len).sum() })
                .sum();
            String::with_capacity(len)
        })
    }
}
