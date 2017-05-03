use super::{ParallelExtend, IntoParallelIterator, ParallelIterator};

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::collections::LinkedList;
use std::collections::{BinaryHeap, VecDeque};

/// Perform a generic `par_extend` by collecting to a `LinkedList<Vec<_>>` in
/// parallel, then extending the collection sequentially.
fn extend<C, I, F>(collection: &mut C, par_iter: I, reserve: F)
    where I: IntoParallelIterator,
          F: FnOnce(&mut C, &LinkedList<Vec<I::Item>>),
          C: Extend<I::Item>
{
    let list = par_iter
        .into_par_iter()
        .fold(Vec::new, |mut vec, elem| {
            vec.push(elem);
            vec
        })
        .collect();

    reserve(collection, &list);
    for vec in list {
        collection.extend(vec);
    }
}

/// Compute the total length of a `LinkedList<Vec<_>>`.
fn len<T>(list: &LinkedList<Vec<T>>) -> usize {
    list.iter().map(Vec::len).sum()
}

/// Compute the total string length of a `LinkedList<Vec<AsRef<str>>>`.
fn str_len<T>(list: &LinkedList<Vec<T>>) -> usize
    where T: AsRef<str>
{
    list.iter()
        .flat_map(|vec| vec.iter().map(|s| s.as_ref().len()))
        .sum()
}


/// Extend a binary heap with items from a parallel iterator.
impl<T> ParallelExtend<T> for BinaryHeap<T>
    where T: Ord + Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = T>
    {
        extend(self, par_iter, |heap, list| heap.reserve(len(list)));
    }
}

/// Extend a binary heap with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for BinaryHeap<T>
    where T: 'a + Copy + Ord + Send + Sync
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = &'a T>
    {
        extend(self, par_iter, |heap, list| heap.reserve(len(list)));
    }
}


/// Extend a B-tree map with items from a parallel iterator.
impl<K, V> ParallelExtend<(K, V)> for BTreeMap<K, V>
    where K: Ord + Send,
          V: Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = (K, V)>
    {
        extend(self, par_iter, |_, _| {});
    }
}

/// Extend a B-tree map with copied items from a parallel iterator.
impl<'a, K, V> ParallelExtend<(&'a K, &'a V)> for BTreeMap<K, V>
    where K: Copy + Ord + Send + Sync,
          V: Copy + Send + Sync
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = (&'a K, &'a V)>
    {
        extend(self, par_iter, |_, _| {});
    }
}


/// Extend a B-tree set with items from a parallel iterator.
impl<T> ParallelExtend<T> for BTreeSet<T>
    where T: Ord + Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = T>
    {
        extend(self, par_iter, |_, _| {});
    }
}

/// Extend a B-tree set with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for BTreeSet<T>
    where T: 'a + Copy + Ord + Send + Sync
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = &'a T>
    {
        extend(self, par_iter, |_, _| {});
    }
}


/// Extend a hash map with items from a parallel iterator.
impl<K, V, S> ParallelExtend<(K, V)> for HashMap<K, V, S>
    where K: Eq + Hash + Send,
          V: Send,
          S: BuildHasher + Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = (K, V)>
    {
        // See the map_collect benchmarks in rayon-demo for different strategies.
        extend(self, par_iter, |map, list| map.reserve(len(list)));
    }
}

/// Extend a hash map with copied items from a parallel iterator.
impl<'a, K, V, S> ParallelExtend<(&'a K, &'a V)> for HashMap<K, V, S>
    where K: Copy + Eq + Hash + Send + Sync,
          V: Copy + Send + Sync,
          S: BuildHasher + Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = (&'a K, &'a V)>
    {
        extend(self, par_iter, |map, list| map.reserve(len(list)));
    }
}


/// Extend a hash set with items from a parallel iterator.
impl<T, S> ParallelExtend<T> for HashSet<T, S>
    where T: Eq + Hash + Send,
          S: BuildHasher + Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = T>
    {
        extend(self, par_iter, |set, list| set.reserve(len(list)));
    }
}

/// Extend a hash set with copied items from a parallel iterator.
impl<'a, T, S> ParallelExtend<&'a T> for HashSet<T, S>
    where T: 'a + Copy + Eq + Hash + Send + Sync,
          S: BuildHasher + Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = &'a T>
    {
        extend(self, par_iter, |set, list| set.reserve(len(list)));
    }
}


/// Extend a linked list with items from a parallel iterator.
impl<T> ParallelExtend<T> for LinkedList<T>
    where T: Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = T>
    {
        let mut list = par_iter
            .into_par_iter()
            .fold(LinkedList::new, |mut list, elem| {
                list.push_back(elem);
                list
            })
            .reduce(LinkedList::new, |mut list1, mut list2| {
                list1.append(&mut list2);
                list1
            });
        self.append(&mut list);
    }
}


/// Extend a linked list with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for LinkedList<T>
    where T: 'a + Copy + Send + Sync
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = &'a T>
    {
        self.par_extend(par_iter.into_par_iter().cloned())
    }
}


/// Extend a string with characters from a parallel iterator.
impl ParallelExtend<char> for String {
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = char>
    {
        // This is like `extend`, but `Vec<char>` is less efficient to deal
        // with than `String`, so instead collect to `LinkedList<String>`.
        let list: LinkedList<_> = par_iter
            .into_par_iter()
            .fold(String::new, |mut string, ch| {
                string.push(ch);
                string
            })
            .collect();

        self.reserve(list.iter().map(String::len).sum());
        self.extend(list)
    }
}

/// Extend a string with copied characters from a parallel iterator.
impl<'a> ParallelExtend<&'a char> for String {
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = &'a char>
    {
        self.par_extend(par_iter.into_par_iter().cloned())
    }
}

/// Extend a string with string slices from a parallel iterator.
impl<'a> ParallelExtend<&'a str> for String {
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = &'a str>
    {
        extend(self, par_iter, |string, list| string.reserve(str_len(list)));
    }
}

/// Extend a string with strings from a parallel iterator.
impl ParallelExtend<String> for String {
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = String>
    {
        extend(self, par_iter, |string, list| string.reserve(str_len(list)));
    }
}


/// Extend a deque with items from a parallel iterator.
impl<T> ParallelExtend<T> for VecDeque<T>
    where T: Send
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = T>
    {
        extend(self, par_iter, |deque, list| deque.reserve(len(list)));
    }
}

/// Extend a deque with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for VecDeque<T>
    where T: 'a + Copy + Send + Sync
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = &'a T>
    {
        extend(self, par_iter, |deque, list| deque.reserve(len(list)));
    }
}


// See the `collect` module for the `Vec<T>` implementation.
// impl<T> ParallelExtend<T> for Vec<T>

/// Extend a vector with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for Vec<T>
    where T: 'a + Copy + Send + Sync
{
    fn par_extend<I>(&mut self, par_iter: I)
        where I: IntoParallelIterator<Item = &'a T>
    {
        self.par_extend(par_iter.into_par_iter().cloned())
    }
}
