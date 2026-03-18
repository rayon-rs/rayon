use crate::iter::plumbing::*;
use crate::iter::*;

/// Parallel iterator over immutable overlapping windows of a slice
#[derive(Debug)]
pub struct Windows<'data, T> {
    window_size: usize,
    slice: &'data [T],
}

impl<'data, T> Windows<'data, T> {
    pub(super) fn new(window_size: usize, slice: &'data [T]) -> Self {
        Self { window_size, slice }
    }
}

impl<T> Clone for Windows<'_, T> {
    fn clone(&self) -> Self {
        Windows { ..*self }
    }
}

impl<'data, T: Sync> ParallelIterator for Windows<'data, T> {
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<T: Sync> IndexedParallelIterator for Windows<'_, T> {
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        assert!(self.window_size >= 1);
        self.slice.len().saturating_sub(self.window_size - 1)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        callback.callback(WindowsProducer {
            window_size: self.window_size,
            slice: self.slice,
        })
    }
}

struct WindowsProducer<'data, T: Sync> {
    window_size: usize,
    slice: &'data [T],
}

impl<'data, T: Sync> Producer for WindowsProducer<'data, T> {
    type Item = &'data [T];
    type IntoIter = ::std::slice::Windows<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.windows(self.window_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let left_index = Ord::min(self.slice.len(), index + (self.window_size - 1));
        let left = &self.slice[..left_index];
        let right = &self.slice[index..];
        (
            WindowsProducer {
                window_size: self.window_size,
                slice: left,
            },
            WindowsProducer {
                window_size: self.window_size,
                slice: right,
            },
        )
    }
}
