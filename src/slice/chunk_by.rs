use crate::iter::plumbing::*;
use crate::iter::*;
use std::fmt;

fn find_first_index<T, P>(xs: &[T], pred: &P) -> Option<usize>
where
    P: Fn(&T, &T) -> bool,
{
    xs.windows(2)
        .position(|w| !pred(&w[0], &w[1]))
        .map(|i| i + 1)
}

fn find_index<T, P>(xs: &[T], pred: &P) -> Option<usize>
where
    P: Fn(&T, &T) -> bool,
{
    let n = (xs.len() / 2).saturating_sub(1);

    for m in (1..((n / 2) + 1)).map(|x| 2 * x) {
        let start = n.saturating_sub(m);
        let end = std::cmp::min(n + m, xs.len());
        let fsts = &xs[start..end];
        let (_, snds) = fsts.split_first()?;
        match fsts.iter().zip(snds).position(|(x, y)| !pred(x, y)) {
            None => (),
            Some(i) => return Some(start + i + 1),
        }
    }
    None
}

struct ChunkByProducer<'data, 'p, T, P> {
    pred: &'p P,
    slice: &'data [T],
}

impl<'data, 'p, T, P> UnindexedProducer for ChunkByProducer<'data, 'p, T, P>
where
    T: Sync,
    P: Fn(&T, &T) -> bool + Send + Sync,
{
    type Item = &'data [T];

    fn split(self) -> (Self, Option<Self>) {
        match find_index(self.slice, self.pred) {
            Some(i) => {
                let (ys, zs) = self.slice.split_at(i);
                (
                    Self {
                        pred: self.pred,
                        slice: ys,
                    },
                    Some(Self {
                        pred: self.pred,
                        slice: zs,
                    }),
                )
            }
            None => (self, None),
        }
    }

    fn fold_with<F>(mut self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        // TODO (MSRV 1.77):
        // folder.consume_iter(self.slice.chunk_by(self.pred))

        folder.consume_iter(std::iter::from_fn(move || {
            if self.slice.is_empty() {
                None
            } else {
                let i = find_first_index(self.slice, self.pred).unwrap_or(self.slice.len());
                let (head, tail) = self.slice.split_at(i);
                self.slice = tail;
                Some(head)
            }
        }))
    }
}

/// Parallel iterator over slice in (non-overlapping) chunks separated by a predicate.
///
/// This struct is created by the [`par_chunk_by`] method on `&[T]`.
///
/// [`par_chunk_by`]: trait.ParallelSlice.html#method.par_chunk_by
pub struct ChunkBy<'data, T, P> {
    pred: P,
    slice: &'data [T],
}

impl<'data, T, P: Clone> Clone for ChunkBy<'data, T, P> {
    fn clone(&self) -> Self {
        ChunkBy {
            pred: self.pred.clone(),
            slice: self.slice,
        }
    }
}

impl<'data, T: fmt::Debug, P> fmt::Debug for ChunkBy<'data, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkBy")
            .field("slice", &self.slice)
            .finish()
    }
}

impl<'data, T, P> ChunkBy<'data, T, P> {
    pub(super) fn new(slice: &'data [T], pred: P) -> Self {
        Self { pred, slice }
    }
}

impl<'data, T, P> ParallelIterator for ChunkBy<'data, T, P>
where
    T: Sync,
    P: Fn(&T, &T) -> bool + Send + Sync,
{
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(
            ChunkByProducer {
                pred: &self.pred,
                slice: self.slice,
            },
            consumer,
        )
    }
}

// Mutable

struct ChunkByMutProducer<'data, 'p, T, P> {
    pred: &'p P,
    slice: &'data mut [T],
}

impl<'data, 'p, T, P> UnindexedProducer for ChunkByMutProducer<'data, 'p, T, P>
where
    T: Send,
    P: Fn(&T, &T) -> bool + Send + Sync,
{
    type Item = &'data mut [T];

    fn split(self) -> (Self, Option<Self>) {
        match find_index(self.slice, self.pred) {
            Some(i) => {
                let (ys, zs) = self.slice.split_at_mut(i);
                (
                    Self {
                        pred: self.pred,
                        slice: ys,
                    },
                    Some(Self {
                        pred: self.pred,
                        slice: zs,
                    }),
                )
            }
            None => (self, None),
        }
    }

    fn fold_with<F>(mut self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        // TODO (MSRV 1.77):
        // folder.consume_iter(self.slice.chunk_by_mut(self.pred))

        folder.consume_iter(std::iter::from_fn(move || {
            if self.slice.is_empty() {
                None
            } else {
                let i = find_first_index(self.slice, self.pred).unwrap_or(self.slice.len());
                let (head, tail) = std::mem::take(&mut self.slice).split_at_mut(i);
                self.slice = tail;
                Some(head)
            }
        }))
    }
}

/// Parallel iterator over slice in (non-overlapping) mutable chunks
/// separated by a predicate.
///
/// This struct is created by the [`par_chunk_by_mut`] method on `&mut [T]`.
///
/// [`par_chunk_by_mut`]: trait.ParallelSliceMut.html#method.par_chunk_by_mut
pub struct ChunkByMut<'data, T, P> {
    pred: P,
    slice: &'data mut [T],
}

impl<'data, T: fmt::Debug, P> fmt::Debug for ChunkByMut<'data, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkByMut")
            .field("slice", &self.slice)
            .finish()
    }
}

impl<'data, T, P> ChunkByMut<'data, T, P> {
    pub(super) fn new(slice: &'data mut [T], pred: P) -> Self {
        Self { pred, slice }
    }
}

impl<'data, T, P> ParallelIterator for ChunkByMut<'data, T, P>
where
    T: Send,
    P: Fn(&T, &T) -> bool + Send + Sync,
{
    type Item = &'data mut [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(
            ChunkByMutProducer {
                pred: &self.pred,
                slice: self.slice,
            },
            consumer,
        )
    }
}
