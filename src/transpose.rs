//! This module contains the parallel iterator type for transposing a
//! HashMap of Vecs into a Vec of HashMaps.  Use HashMapVecTranspose::new
//! to operate the iterator.

use crate::iter::plumbing::*;
use crate::iter::*;
use crate::vec::{DrainProducer, SliceDrain};
use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};
use std::iter;

/// Parallel iterator that clones K and moves V and yields `HashMap<K, V>` out of a hashmap of vectors.
#[derive(Debug, Clone)]
pub struct HashMapVecTranspose<K, V, S> {
    map: HashMap<K, Vec<V>, S>,
    len: usize,
}

impl<K, V, S> HashMapVecTranspose<K, V, S> {
    /// Create a new HashMapVecTranspose.  All Vecs must be the same length.
    pub fn new(map: HashMap<K, Vec<V>, S>, len: usize) -> Self {
        Self { map, len }
    }
}

impl<K, V, S> ParallelIterator for HashMapVecTranspose<K, V, S>
where
    K: Send + Clone + Eq + Hash,
    V: Send + Clone,
    S: Send + Clone + BuildHasher + Default,
{
    type Item = HashMap<K, V, S>;

    fn drive_unindexed<C: UnindexedConsumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len)
    }
}

impl<K, V, S> IndexedParallelIterator for HashMapVecTranspose<K, V, S>
where
    K: Send + Clone + Eq + Hash,
    V: Send + Clone,
    S: Send + Clone + BuildHasher + Default,
{
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.len
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(mut self, callback: CB) -> CB::Output {
        // Allocate the HashMaps single-threaded to avoid allocator lock contention
        let mut hash_maps =
            vec![HashMap::with_capacity_and_hasher(self.map.len(), S::default()); self.len];

        // Create the producer as the exclusive "owner" of the slice.
        let producer = TransposeProducer::from_transpose(&mut self, &mut hash_maps);

        // The producer will move or drop each item from the drained range.
        callback.callback(producer)
    }
}

// ////////////////////////////////////////////////////////////////////////
struct TransposeProducer<'data, K: Send, V: Send, S: Send> {
    map: HashMap<K, DrainProducer<'data, V>, S>,
    hash_maps: DrainProducer<'data, HashMap<K, V, S>>,
}

impl<'data, K, V, S> TransposeProducer<'data, K, V, S>
where
    K: Send + Clone + Eq + Hash,
    V: Send,
    S: Send + BuildHasher + Default,
{
    fn new(
        map: HashMap<K, DrainProducer<'data, V>, S>,
        hash_maps: DrainProducer<'data, HashMap<K, V, S>>,
    ) -> Self {
        Self { map, hash_maps }
    }

    fn from_transpose(
        transpose: &'data mut HashMapVecTranspose<K, V, S>,
        hash_maps: &'data mut Vec<HashMap<K, V, S>>,
    ) -> Self {
        let len = transpose.len;
        let map = transpose
            .map
            .iter_mut()
            .map(|(key, vec)| {
                (key.clone(), unsafe {
                    vec.set_len(0);
                    DrainProducer::from_vec(vec, len)
                })
            })
            .collect();
        let hash_maps = unsafe {
            hash_maps.set_len(0);
            DrainProducer::from_vec(hash_maps, len)
        };
        Self::new(map, hash_maps)
    }
}

impl<'data, K, V, S> Producer for TransposeProducer<'data, K, V, S>
where
    K: 'data + Send + Clone + Eq + Hash,
    V: 'data + Send,
    S: 'data + Send + BuildHasher + Default,
{
    type Item = HashMap<K, V, S>;
    type IntoIter = TransposeSliceDrain<'data, K, V, S>;

    fn into_iter(self) -> Self::IntoIter {
        let map = self
            .map
            .into_iter()
            .map(|(key, drain_producer)| (key, drain_producer.into_iter()))
            .collect();
        let hash_maps = self.hash_maps.into_iter();
        TransposeSliceDrain { map, hash_maps }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        // TODO: figure out if there is a way to reuse the allocation of self
        let (m_left, m_right) = self
            .map
            .into_iter()
            .map(|(key, drain_producer)| {
                let (left, right) = drain_producer.split_at(index);
                ((key.clone(), left), (key, right))
            })
            .collect();
        let (h_left, h_right) = self.hash_maps.split_at(index);
        (Self::new(m_left, h_left), Self::new(m_right, h_right))
    }
}

// ////////////////////////////////////////////////////////////////////////

// like std::vec::Drain, without updating a source Vec
struct TransposeSliceDrain<'data, K, V, S> {
    map: HashMap<K, SliceDrain<'data, V>, S>,
    hash_maps: SliceDrain<'data, HashMap<K, V, S>>,
}

impl<'data, K, V, S> Iterator for TransposeSliceDrain<'data, K, V, S>
where
    K: Clone + Eq + Hash,
    V: 'data,
    S: BuildHasher + Default,
{
    type Item = HashMap<K, V, S>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut hash_map = self.hash_maps.next()?;
        for (key, iter) in &mut self.map {
            hash_map.insert(key.clone(), iter.next()?);
        }
        Some(hash_map)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }

    fn count(self) -> usize {
        self.len()
    }
}

impl<'data, K, V, S> DoubleEndedIterator for TransposeSliceDrain<'data, K, V, S>
where
    K: Clone + Eq + Hash,
    V: 'data,
    S: BuildHasher + Default,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let mut hash_map = self.hash_maps.next_back()?;
        for (key, iter) in &mut self.map {
            hash_map.insert(key.clone(), iter.next_back()?);
        }
        Some(hash_map)
    }
}

impl<'data, K, V, S> ExactSizeIterator for TransposeSliceDrain<'data, K, V, S>
where
    K: Clone + Eq + Hash,
    V: 'data,
    S: BuildHasher + Default,
{
    fn len(&self) -> usize {
        self.hash_maps.len()
    }
}

impl<'data, K, V, S> iter::FusedIterator for TransposeSliceDrain<'data, K, V, S>
where
    K: Clone + Eq + Hash,
    V: 'data,
    S: BuildHasher + Default,
{
}
