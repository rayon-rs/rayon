
use crate::iter::plumbing::*;
use crate::iter::*;

fn find_index<T, F>(xs: &[T], pred: &F) -> Option<usize>
where
    F: Fn(&T, &T) -> bool,
{
    let n = xs.len() / 2;

    for (start, end) in (0..).scan(true, |cont, i| {
        if *cont {
            let offset = 2 * i;
            let start = n.saturating_sub(offset);
            let end = n + offset;
            Some(
                if !(1..xs.len()).contains(&start) || !(2..xs.len()).contains(&end) {
                    *cont = false;
                    (0, xs.len())
                }
                else {
                    (start, end)
                },
            )
        }
        else {
            None
        }
    }) {
        match xs[start..end]
            .windows(2)
            .enumerate()
            .find_map(|(i, win)| {
                if pred(&win[0], &win[1]) {
                    None
                }
                else {
                    Some(i)
                }
            })
            .map(|i| start + i)
        {
            Some(i) => return Some(i),
            None => {}
        }
    }
    None
}

struct GroupByProducer<'data, 'p, T, P>
{
    pred: &'p P,
    slice: &'data [T],
}

impl<'data, 'p, T, P> UnindexedProducer for GroupByProducer<'data, 'p, T, P>
where
    T: Sync,
    P: Fn(&T, &T) -> bool + Send + Sync,
{
    type Item = &'data [T];

    fn split(self) -> (Self, Option<Self>)
    {
        match find_index(self.slice, self.pred) {
            Some(i) => {
                let (ys, zs) = self.slice.split_at(i + 1);
                (Self { pred: self.pred, slice: ys },
                Some(Self { pred: self.pred, slice: zs }))
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
/// This struct is created by the [`group_by`] method on `&[T]`.
///
/// [`group_by`]: trait.ParallelSlice.html#method.par_group_by
#[derive(Debug)]
pub struct GroupBy<'data, T, F>
where
    T: Sync,
    F: Fn(&T, &T) -> bool + Send + Sync
{
    pred: F,
    slice: &'data [T],
}

impl<'data, T, F> GroupBy<'data, T, F> 
where
    T: Sync,
    F: Fn(&T, &T) -> bool + Send + Sync
{
    pub(super) fn new(slice: &'data [T], pred: F) -> Self
    {
        Self { pred, slice }
    }
}


impl<'data, T, F> ParallelIterator for GroupBy<'data, T, F>
where
    T: Sync,
    F: Fn(&T, &T) -> bool + Send + Sync,
{
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(GroupByProducer { pred: &self.pred, slice: self.slice }, consumer)
    }
}
        
