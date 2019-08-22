use super::noop::NoopConsumer;
use super::{IntoParallelIterator, ParallelExtend, ParallelIterator};

use std::borrow::Cow;
use std::collections::LinkedList;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::collections::{BinaryHeap, VecDeque};
use std::hash::{BuildHasher, Hash};

/// Perform a generic `par_extend` by collecting to a `LinkedList<Vec<_>>` in
/// parallel, then extending the collection sequentially.
fn extend<C, I, F>(collection: &mut C, par_iter: I, reserve: F)
where
    I: IntoParallelIterator,
    F: FnOnce(&mut C, &LinkedList<Vec<I::Item>>),
    C: Extend<I::Item>,
{
    let list = collect(par_iter);
    reserve(collection, &list);
    for vec in list {
        collection.extend(vec);
    }
}

pub(super) fn collect<I>(par_iter: I) -> LinkedList<Vec<I::Item>>
where
    I: IntoParallelIterator,
{
    par_iter
        .into_par_iter()
        .fold(Vec::new, vec_push)
        .map(as_list)
        .reduce(LinkedList::new, list_append)
}

fn vec_push<T>(mut vec: Vec<T>, elem: T) -> Vec<T> {
    vec.push(elem);
    vec
}

fn as_list<T>(item: T) -> LinkedList<T> {
    let mut list = LinkedList::new();
    list.push_back(item);
    list
}

fn list_append<T>(mut list1: LinkedList<T>, mut list2: LinkedList<T>) -> LinkedList<T> {
    list1.append(&mut list2);
    list1
}

/// Compute the total length of a `LinkedList<Vec<_>>`.
pub(super) fn len<T>(list: &LinkedList<Vec<T>>) -> usize {
    list.iter().map(Vec::len).sum()
}

fn no_reserve<C, T>(_: &mut C, _: &LinkedList<Vec<T>>) {}

fn heap_reserve<T: Ord, U>(heap: &mut BinaryHeap<T>, list: &LinkedList<Vec<U>>) {
    heap.reserve(len(list));
}

/// Extend a binary heap with items from a parallel iterator.
impl<T> ParallelExtend<T> for BinaryHeap<T>
where
    T: Ord + Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = T>,
    {
        extend(self, par_iter, heap_reserve);
    }
}

/// Extend a binary heap with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for BinaryHeap<T>
where
    T: 'a + Copy + Ord + Send + Sync,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a T>,
    {
        extend(self, par_iter, heap_reserve);
    }
}

/// Extend a B-tree map with items from a parallel iterator.
impl<K, V> ParallelExtend<(K, V)> for BTreeMap<K, V>
where
    K: Ord + Send,
    V: Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = (K, V)>,
    {
        extend(self, par_iter, no_reserve);
    }
}

/// Extend a B-tree map with copied items from a parallel iterator.
impl<'a, K: 'a, V: 'a> ParallelExtend<(&'a K, &'a V)> for BTreeMap<K, V>
where
    K: Copy + Ord + Send + Sync,
    V: Copy + Send + Sync,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = (&'a K, &'a V)>,
    {
        extend(self, par_iter, no_reserve);
    }
}

/// Extend a B-tree set with items from a parallel iterator.
impl<T> ParallelExtend<T> for BTreeSet<T>
where
    T: Ord + Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = T>,
    {
        extend(self, par_iter, no_reserve);
    }
}

/// Extend a B-tree set with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for BTreeSet<T>
where
    T: 'a + Copy + Ord + Send + Sync,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a T>,
    {
        extend(self, par_iter, no_reserve);
    }
}

fn map_reserve<K, V, S, U>(map: &mut HashMap<K, V, S>, list: &LinkedList<Vec<U>>)
where
    K: Eq + Hash,
    S: BuildHasher,
{
    map.reserve(len(list));
}

/// Extend a hash map with items from a parallel iterator.
impl<K, V, S> ParallelExtend<(K, V)> for HashMap<K, V, S>
where
    K: Eq + Hash + Send,
    V: Send,
    S: BuildHasher + Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = (K, V)>,
    {
        // See the map_collect benchmarks in rayon-demo for different strategies.
        extend(self, par_iter, map_reserve);
    }
}

/// Extend a hash map with copied items from a parallel iterator.
impl<'a, K: 'a, V: 'a, S> ParallelExtend<(&'a K, &'a V)> for HashMap<K, V, S>
where
    K: Copy + Eq + Hash + Send + Sync,
    V: Copy + Send + Sync,
    S: BuildHasher + Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = (&'a K, &'a V)>,
    {
        extend(self, par_iter, map_reserve);
    }
}

fn set_reserve<T, S, U>(set: &mut HashSet<T, S>, list: &LinkedList<Vec<U>>)
where
    T: Eq + Hash,
    S: BuildHasher,
{
    set.reserve(len(list));
}

/// Extend a hash set with items from a parallel iterator.
impl<T, S> ParallelExtend<T> for HashSet<T, S>
where
    T: Eq + Hash + Send,
    S: BuildHasher + Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = T>,
    {
        extend(self, par_iter, set_reserve);
    }
}

/// Extend a hash set with copied items from a parallel iterator.
impl<'a, T, S> ParallelExtend<&'a T> for HashSet<T, S>
where
    T: 'a + Copy + Eq + Hash + Send + Sync,
    S: BuildHasher + Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a T>,
    {
        extend(self, par_iter, set_reserve);
    }
}

fn list_push_back<T>(mut list: LinkedList<T>, elem: T) -> LinkedList<T> {
    list.push_back(elem);
    list
}

/// Extend a linked list with items from a parallel iterator.
impl<T> ParallelExtend<T> for LinkedList<T>
where
    T: Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = T>,
    {
        let mut list = par_iter
            .into_par_iter()
            .fold(LinkedList::new, list_push_back)
            .reduce(LinkedList::new, list_append);
        self.append(&mut list);
    }
}

/// Extend a linked list with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for LinkedList<T>
where
    T: 'a + Copy + Send + Sync,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a T>,
    {
        self.par_extend(par_iter.into_par_iter().cloned())
    }
}

fn string_push(mut string: String, ch: char) -> String {
    string.push(ch);
    string
}

/// Extend a string with characters from a parallel iterator.
impl ParallelExtend<char> for String {
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = char>,
    {
        // This is like `extend`, but `Vec<char>` is less efficient to deal
        // with than `String`, so instead collect to `LinkedList<String>`.
        let list: LinkedList<_> = par_iter
            .into_par_iter()
            .fold(String::new, string_push)
            .map(as_list)
            .reduce(LinkedList::new, list_append);

        self.reserve(list.iter().map(String::len).sum());
        self.extend(list)
    }
}

/// Extend a string with copied characters from a parallel iterator.
impl<'a> ParallelExtend<&'a char> for String {
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a char>,
    {
        self.par_extend(par_iter.into_par_iter().cloned())
    }
}

fn string_reserve<T: AsRef<str>>(string: &mut String, list: &LinkedList<Vec<T>>) {
    let len = list
        .iter()
        .flat_map(|vec| vec)
        .map(T::as_ref)
        .map(str::len)
        .sum();
    string.reserve(len);
}

/// Extend a string with string slices from a parallel iterator.
impl<'a> ParallelExtend<&'a str> for String {
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a str>,
    {
        extend(self, par_iter, string_reserve);
    }
}

/// Extend a string with strings from a parallel iterator.
impl ParallelExtend<String> for String {
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = String>,
    {
        extend(self, par_iter, string_reserve);
    }
}

/// Extend a string with string slices from a parallel iterator.
impl<'a> ParallelExtend<Cow<'a, str>> for String {
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = Cow<'a, str>>,
    {
        extend(self, par_iter, string_reserve);
    }
}

fn deque_reserve<T, U>(deque: &mut VecDeque<T>, list: &LinkedList<Vec<U>>) {
    deque.reserve(len(list));
}

/// Extend a deque with items from a parallel iterator.
impl<T> ParallelExtend<T> for VecDeque<T>
where
    T: Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = T>,
    {
        extend(self, par_iter, deque_reserve);
    }
}

/// Extend a deque with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for VecDeque<T>
where
    T: 'a + Copy + Send + Sync,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a T>,
    {
        extend(self, par_iter, deque_reserve);
    }
}

// See the `collect` module for the `Vec<T>` implementation.
// impl<T> ParallelExtend<T> for Vec<T>

/// Extend a vector with copied items from a parallel iterator.
impl<'a, T> ParallelExtend<&'a T> for Vec<T>
where
    T: 'a + Copy + Send + Sync,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a T>,
    {
        self.par_extend(par_iter.into_par_iter().cloned())
    }
}

/// Collapses all unit items from a parallel iterator into one.
impl ParallelExtend<()> for () {
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = ()>,
    {
        par_iter.into_par_iter().drive_unindexed(NoopConsumer)
    }
}
