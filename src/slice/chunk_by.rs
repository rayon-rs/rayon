use crate::iter::plumbing::*;
use crate::iter::*;

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

    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        folder.consume_iter(self.slice.chunk_by(self.pred))
    }
}

/// Parallel iterator over slice in (non-overlapping) chunks separated by a predicate.
///
/// This struct is created by the [`par_chunk_by`] method on `&[T]`.
///
/// [`par_chunk_by`]: trait.ParallelSlice.html#method.par_chunk_by
#[derive(Debug)]
pub struct ChunkBy<'data, T, P> {
    pred: P,
    slice: &'data [T],
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

    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        folder.consume_iter(self.slice.chunk_by_mut(self.pred))
    }
}

/// Parallel iterator over slice in (non-overlapping) mutable chunks
/// separated by a predicate.
///
/// This struct is created by the [`par_chunk_by_mut`] method on `&mut [T]`.
///
/// [`par_chunk_by_mut`]: trait.ParallelSliceMut.html#method.par_chunk_by_mut
#[derive(Debug)]
pub struct ChunkByMut<'data, T, P> {
    pred: P,
    slice: &'data mut [T],
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
