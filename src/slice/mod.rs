//! This module contains the parallel iterator types for slices
//! (`[T]`). You will rarely need to interact with it directly unless
//! you have need to name one of those types.

mod mergesort;
mod quicksort;

mod test;

use iter::*;
use iter::internal::*;
use self::mergesort::par_mergesort;
use self::quicksort::par_quicksort;
use split_producer::*;
use std::cmp;
use std::cmp::Ordering;

/// Parallel extensions for slices.
pub trait ParallelSlice<T: Sync> {
    /// Returns a plain slice, which is used to implement the rest of the
    /// parallel methods.
    fn as_parallel_slice(&self) -> &[T];

    /// Returns a parallel iterator over subslices separated by elements that
    /// match the separator.
    fn par_split<P>(&self, separator: P) -> Split<T, P>
        where P: Fn(&T) -> bool + Sync + Send
    {
        Split {
            slice: self.as_parallel_slice(),
            separator: separator,
        }
    }

    /// Returns a parallel iterator over all contiguous windows of
    /// length `size`. The windows overlap.
    fn par_windows(&self, window_size: usize) -> Windows<T> {
        Windows {
            window_size: window_size,
            slice: self.as_parallel_slice(),
        }
    }

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks do not overlap.
    fn par_chunks(&self, chunk_size: usize) -> Chunks<T> {
        Chunks {
            chunk_size: chunk_size,
            slice: self.as_parallel_slice(),
        }
    }
}

impl<T: Sync> ParallelSlice<T> for [T] {
    #[inline]
    fn as_parallel_slice(&self) -> &[T] {
        self
    }
}


/// Parallel extensions for mutable slices.
pub trait ParallelSliceMut<T: Send> {
    /// Returns a plain mutable slice, which is used to implement the rest of
    /// the parallel methods.
    fn as_parallel_slice_mut(&mut self) -> &mut [T];

    /// Returns a parallel iterator over mutable subslices separated by
    /// elements that match the separator.
    fn par_split_mut<P>(&mut self, separator: P) -> SplitMut<T, P>
        where P: Fn(&T) -> bool + Sync + Send
    {
        SplitMut {
            slice: self.as_parallel_slice_mut(),
            separator: separator,
        }
    }

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks are mutable and do not overlap.
    fn par_chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<T> {
        ChunksMut {
            chunk_size: chunk_size,
            slice: self.as_parallel_slice_mut(),
        }
    }

    /// Sorts the slice in parallel.
    ///
    /// This sort is stable (i.e. does not reorder equal elements) and `O(n log n)` worst-case.
    ///
    /// When applicable, unstable sorting is preferred because it is generally faster than stable
    /// sorting and it doesn't allocate auxiliary memory.
    /// See [`par_sort_unstable`](#method.par_sort_unstable).
    ///
    /// # Current implementation
    ///
    /// The current algorithm is an adaptive merge sort inspired by
    /// [timsort](https://en.wikipedia.org/wiki/Timsort).
    /// It is designed to be very fast in cases where the slice is nearly sorted, or consists of
    /// two or more sorted sequences concatenated one after another.
    ///
    /// Also, it allocates temporary storage the same size as `self`, but for very short slices a
    /// non-allocating insertion sort is used instead.
    ///
    /// In order to sort the slice in parallel, the slice is first divided into smaller chunks and
    /// all chunks are sorted in parallel. Then, adjacent chunks that together form non-descending
    /// or descending runs are concatenated. Finally, the remaining chunks are merged together using
    /// parallel subdivision of chunks and parallel merge operation.
    fn par_sort(&mut self)
    where
        T: Ord,
    {
        par_mergesort(self.as_parallel_slice_mut(), |a, b| a.lt(b));
    }

    /// Sorts the slice in parallel with a comparator function.
    ///
    /// This sort is stable (i.e. does not reorder equal elements) and `O(n log n)` worst-case.
    ///
    /// When applicable, unstable sorting is preferred because it is generally faster than stable
    /// sorting and it doesn't allocate auxiliary memory.
    /// See [`par_sort_unstable_by`](#method.par_sort_unstable_by).
    ///
    /// # Current implementation
    ///
    /// The current algorithm is an adaptive merge sort inspired by
    /// [timsort](https://en.wikipedia.org/wiki/Timsort).
    /// It is designed to be very fast in cases where the slice is nearly sorted, or consists of
    /// two or more sorted sequences concatenated one after another.
    ///
    /// Also, it allocates temporary storage the same size as `self`, but for very short slices a
    /// non-allocating insertion sort is used instead.
    ///
    /// In order to sort the slice in parallel, the slice is first divided into smaller chunks and
    /// all chunks are sorted in parallel. Then, adjacent chunks that together form non-descending
    /// or descending runs are concatenated. Finally, the remaining chunks are merged together using
    /// parallel subdivision of chunks and parallel merge operation.
    fn par_sort_by<F>(&mut self, compare: F)
    where
        F: Fn(&T, &T) -> Ordering + Sync,
    {
        par_mergesort(self.as_parallel_slice_mut(), |a, b| compare(a, b) == Ordering::Less);
    }

    /// Sorts the slice in parallel with a key extraction function.
    ///
    /// This sort is stable (i.e. does not reorder equal elements) and `O(n log n)` worst-case.
    ///
    /// When applicable, unstable sorting is preferred because it is generally faster than stable
    /// sorting and it doesn't allocate auxiliary memory.
    /// See [`par_sort_unstable_by_key`](#method.par_sort_unstable_by_key).
    ///
    /// # Current implementation
    ///
    /// The current algorithm is an adaptive merge sort inspired by
    /// [timsort](https://en.wikipedia.org/wiki/Timsort).
    /// It is designed to be very fast in cases where the slice is nearly sorted, or consists of
    /// two or more sorted sequences concatenated one after another.
    ///
    /// Also, it allocates temporary storage the same size as `self`, but for very short slices a
    /// non-allocating insertion sort is used instead.
    ///
    /// In order to sort the slice in parallel, the slice is first divided into smaller chunks and
    /// all chunks are sorted in parallel. Then, adjacent chunks that together form non-descending
    /// or descending runs are concatenated. Finally, the remaining chunks are merged together using
    /// parallel subdivision of chunks and parallel merge operation.
    fn par_sort_by_key<B, F>(&mut self, f: F)
    where
        B: Ord,
        F: Fn(&T) -> B + Sync,
    {
        par_mergesort(self.as_parallel_slice_mut(), |a, b| f(a).lt(&f(b)));
    }

    /// Sorts the slice in parallel, but may not preserve the order of equal elements.
    ///
    /// This sort is unstable (i.e. may reorder equal elements), in-place (i.e. does not allocate),
    /// and `O(n log n)` worst-case.
    ///
    /// # Current implementation
    ///
    /// The current algorithm is based on Orson Peters' [pattern-defeating quicksort][pdqsort],
    /// which is a quicksort variant designed to be very fast on certain kinds of patterns,
    /// sometimes achieving linear time. It is randomized but deterministic, and falls back to
    /// heapsort on degenerate inputs.
    ///
    /// It is generally faster than stable sorting, except in a few special cases, e.g. when the
    /// slice consists of several concatenated sorted sequences.
    ///
    /// All quicksorts work in two stages: partitioning into two halves followed by recursive
    /// calls. The partitioning phase is sequential, but the two recursive calls are performed in
    /// parallel.
    ///
    /// [pdqsort]: https://github.com/orlp/pdqsort
    fn par_sort_unstable(&mut self)
    where
        T: Ord,
    {
        par_quicksort(self.as_parallel_slice_mut(), |a, b| a.lt(b));
    }

    /// Sorts the slice in parallel with a comparator function, but may not preserve the order of
    /// equal elements.
    ///
    /// This sort is unstable (i.e. may reorder equal elements), in-place (i.e. does not allocate),
    /// and `O(n log n)` worst-case.
    ///
    /// # Current implementation
    ///
    /// The current algorithm is based on Orson Peters' [pattern-defeating quicksort][pdqsort],
    /// which is a quicksort variant designed to be very fast on certain kinds of patterns,
    /// sometimes achieving linear time. It is randomized but deterministic, and falls back to
    /// heapsort on degenerate inputs.
    ///
    /// It is generally faster than stable sorting, except in a few special cases, e.g. when the
    /// slice consists of several concatenated sorted sequences.
    ///
    /// All quicksorts work in two stages: partitioning into two halves followed by recursive
    /// calls. The partitioning phase is sequential, but the two recursive calls are performed in
    /// parallel.
    ///
    /// [pdqsort]: https://github.com/orlp/pdqsort
    fn par_sort_unstable_by<F>(&mut self, compare: F)
    where
        F: Fn(&T, &T) -> Ordering + Sync,
    {
        par_quicksort(self.as_parallel_slice_mut(), |a, b| compare(a, b) == Ordering::Less);
    }

    /// Sorts the slice in parallel with a key extraction function, but may not preserve the order
    /// of equal elements.
    ///
    /// This sort is unstable (i.e. may reorder equal elements), in-place (i.e. does not allocate),
    /// and `O(n log n)` worst-case.
    ///
    /// # Current implementation
    ///
    /// The current algorithm is based on Orson Peters' [pattern-defeating quicksort][pdqsort],
    /// which is a quicksort variant designed to be very fast on certain kinds of patterns,
    /// sometimes achieving linear time. It is randomized but deterministic, and falls back to
    /// heapsort on degenerate inputs.
    ///
    /// It is generally faster than stable sorting, except in a few special cases, e.g. when the
    /// slice consists of several concatenated sorted sequences.
    ///
    /// All quicksorts work in two stages: partitioning into two halves followed by recursive
    /// calls. The partitioning phase is sequential, but the two recursive calls are performed in
    /// parallel.
    ///
    /// [pdqsort]: https://github.com/orlp/pdqsort
    fn par_sort_unstable_by_key<B, F>(&mut self, f: F)
    where
        B: Ord,
        F: Fn(&T) -> B + Sync,
    {
        par_quicksort(self.as_parallel_slice_mut(), |a, b| f(a).lt(&f(b)));
    }
}

impl<T: Send> ParallelSliceMut<T> for [T] {
    #[inline]
    fn as_parallel_slice_mut(&mut self) -> &mut [T] {
        self
    }
}


impl<'data, T: Sync + 'data> IntoParallelIterator for &'data [T] {
    type Item = &'data T;
    type Iter = Iter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { slice: self }
    }
}

impl<'data, T: Sync + 'data> IntoParallelIterator for &'data Vec<T> {
    type Item = &'data T;
    type Iter = Iter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { slice: self }
    }
}

impl<'data, T: Send + 'data> IntoParallelIterator for &'data mut [T] {
    type Item = &'data mut T;
    type Iter = IterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        IterMut { slice: self }
    }
}

impl<'data, T: Send + 'data> IntoParallelIterator for &'data mut Vec<T> {
    type Item = &'data mut T;
    type Iter = IterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        IterMut { slice: self }
    }
}


/// Parallel iterator over immutable items in a slice
pub struct Iter<'data, T: 'data + Sync> {
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Iter<'data, T> {
    type Item = &'data T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Iter<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        self.slice.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(IterProducer { slice: self.slice })
    }
}

struct IterProducer<'data, T: 'data + Sync> {
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for IterProducer<'data, T> {
    type Item = &'data T;
    type IntoIter = ::std::slice::Iter<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at(index);
        (IterProducer { slice: left }, IterProducer { slice: right })
    }
}


/// Parallel iterator over immutable non-overlapping chunks of a slice
pub struct Chunks<'data, T: 'data + Sync> {
    chunk_size: usize,
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Chunks<'data, T> {
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Chunks<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        (self.slice.len() + (self.chunk_size - 1)) / self.chunk_size
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(ChunksProducer {
                              chunk_size: self.chunk_size,
                              slice: self.slice,
                          })
    }
}

struct ChunksProducer<'data, T: 'data + Sync> {
    chunk_size: usize,
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for ChunksProducer<'data, T> {
    type Item = &'data [T];
    type IntoIter = ::std::slice::Chunks<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks(self.chunk_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at(elem_index);
        (ChunksProducer {
             chunk_size: self.chunk_size,
             slice: left,
         },
         ChunksProducer {
             chunk_size: self.chunk_size,
             slice: right,
         })
    }
}


/// Parallel iterator over immutable overlapping windows of a slice
pub struct Windows<'data, T: 'data + Sync> {
    window_size: usize,
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Windows<'data, T> {
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Windows<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        assert!(self.window_size >= 1);
        self.slice.len().saturating_sub(self.window_size - 1)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(WindowsProducer {
                              window_size: self.window_size,
                              slice: self.slice,
                          })
    }
}

struct WindowsProducer<'data, T: 'data + Sync> {
    window_size: usize,
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for WindowsProducer<'data, T> {
    type Item = &'data [T];
    type IntoIter = ::std::slice::Windows<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.windows(self.window_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let left_index = cmp::min(self.slice.len(), index + (self.window_size - 1));
        let left = &self.slice[..left_index];
        let right = &self.slice[index..];
        (WindowsProducer {
             window_size: self.window_size,
             slice: left,
         },
         WindowsProducer {
             window_size: self.window_size,
             slice: right,
         })
    }
}


/// Parallel iterator over mutable items in a slice
pub struct IterMut<'data, T: 'data + Send> {
    slice: &'data mut [T],
}

impl<'data, T: Send + 'data> ParallelIterator for IterMut<'data, T> {
    type Item = &'data mut T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Send + 'data> IndexedParallelIterator for IterMut<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        self.slice.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(IterMutProducer { slice: self.slice })
    }
}

struct IterMutProducer<'data, T: 'data + Send> {
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for IterMutProducer<'data, T> {
    type Item = &'data mut T;
    type IntoIter = ::std::slice::IterMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at_mut(index);
        (IterMutProducer { slice: left }, IterMutProducer { slice: right })
    }
}


/// Parallel iterator over mutable non-overlapping chunks of a slice
pub struct ChunksMut<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T],
}

impl<'data, T: Send + 'data> ParallelIterator for ChunksMut<'data, T> {
    type Item = &'data mut [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Send + 'data> IndexedParallelIterator for ChunksMut<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        (self.slice.len() + (self.chunk_size - 1)) / self.chunk_size
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(ChunksMutProducer {
                              chunk_size: self.chunk_size,
                              slice: self.slice,
                          })
    }
}

struct ChunksMutProducer<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for ChunksMutProducer<'data, T> {
    type Item = &'data mut [T];
    type IntoIter = ::std::slice::ChunksMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks_mut(self.chunk_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at_mut(elem_index);
        (ChunksMutProducer {
             chunk_size: self.chunk_size,
             slice: left,
         },
         ChunksMutProducer {
             chunk_size: self.chunk_size,
             slice: right,
         })
    }
}


/// Parallel iterator over slices separated by a predicate
pub struct Split<'data, T: 'data, P> {
    slice: &'data [T],
    separator: P,
}

impl<'data, T, P> ParallelIterator for Split<'data, T, P>
    where P: Fn(&T) -> bool + Sync + Send,
          T: Sync
{
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitProducer::new(self.slice, &self.separator);
        bridge_unindexed(producer, consumer)
    }
}

/// Implement support for `SplitProducer`.
impl<'data, T, P> Fissile<P> for &'data [T]
    where P: Fn(&T) -> bool
{
    fn length(&self) -> usize {
        self.len()
    }

    fn midpoint(&self, end: usize) -> usize {
        end / 2
    }

    fn find(&self, separator: &P, start: usize, end: usize) -> Option<usize> {
        self[start..end].iter().position(separator)
    }

    fn rfind(&self, separator: &P, end: usize) -> Option<usize> {
        self[..end].iter().rposition(separator)
    }

    fn split_once(self, index: usize) -> (Self, Self) {
        let (left, right) = self.split_at(index);
        (left, &right[1..]) // skip the separator
    }

    fn fold_splits<F>(self, separator: &P, folder: F, skip_last: bool) -> F
        where F: Folder<Self>,
              Self: Send
    {
        let mut split = self.split(separator);
        if skip_last {
            split.next_back();
        }
        folder.consume_iter(split)
    }
}


/// Parallel iterator over mutable slices separated by a predicate
pub struct SplitMut<'data, T: 'data, P> {
    slice: &'data mut [T],
    separator: P,
}

impl<'data, T, P> ParallelIterator for SplitMut<'data, T, P>
    where P: Fn(&T) -> bool + Sync + Send,
          T: Send
{
    type Item = &'data mut [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitProducer::new(self.slice, &self.separator);
        bridge_unindexed(producer, consumer)
    }
}

/// Implement support for `SplitProducer`.
impl<'data, T, P> Fissile<P> for &'data mut [T]
    where P: Fn(&T) -> bool
{
    fn length(&self) -> usize {
        self.len()
    }

    fn midpoint(&self, end: usize) -> usize {
        end / 2
    }

    fn find(&self, separator: &P, start: usize, end: usize) -> Option<usize> {
        self[start..end].iter().position(separator)
    }

    fn rfind(&self, separator: &P, end: usize) -> Option<usize> {
        self[..end].iter().rposition(separator)
    }

    fn split_once(self, index: usize) -> (Self, Self) {
        let (left, right) = self.split_at_mut(index);
        (left, &mut right[1..]) // skip the separator
    }

    fn fold_splits<F>(self, separator: &P, folder: F, skip_last: bool) -> F
        where F: Folder<Self>,
              Self: Send
    {
        let mut split = self.split_mut(separator);
        if skip_last {
            split.next_back();
        }
        folder.consume_iter(split)
    }
}
