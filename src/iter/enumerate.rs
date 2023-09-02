use super::plumbing::*;
use super::*;

/// `Enumerate` is an iterator that returns the current count along with the element.
/// This struct is created by the [`enumerate()`] method on [`IndexedParallelIterator`]
///
/// [`enumerate()`]: trait.IndexedParallelIterator.html#method.enumerate
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct Enumerate<I: IndexedParallelIterator> {
    base: I,
}

impl<I> Enumerate<I>
where
    I: IndexedParallelIterator,
{
    /// Creates a new `Enumerate` iterator.
    #[inline]
    pub(super) fn new(base: I) -> Self {
        Enumerate { base }
    }
}

impl<I> ParallelIterator for Enumerate<I>
where
    I: IndexedParallelIterator,
{
    type Item = (usize, I::Item);

    #[inline]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    #[inline]
    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I> IndexedParallelIterator for Enumerate<I>
where
    I: IndexedParallelIterator,
{
    #[inline]
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }

    #[inline]
    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        return self.base.with_producer(Callback { callback });

        struct Callback<CB> {
            callback: CB,
        }

        impl<I, CB> ProducerCallback<I> for Callback<CB>
        where
            CB: ProducerCallback<(usize, I)>,
        {
            type Output = CB::Output;
            fn callback<P>(self, base: P) -> CB::Output
            where
                P: Producer<Item = I>,
            {
                let producer = EnumerateAdapter { base, offset: 0 };
                self.callback.callback(producer)
            }
        }
    }
}

struct EnumerateAdapter<T> {
    base: T,
    offset: usize,
}

/// ////////////////////////////////////////////////////////////////////////
/// Producer implementation

impl<P> Producer for EnumerateAdapter<P>
where
    P: Producer,
{
    type Item = (usize, P::Item);
    type IntoIter = EnumerateAdapter<P::IntoIter>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        EnumerateAdapter {
            base: self.base.into_iter(),
            offset: self.offset,
        }
    }

    #[inline]
    fn min_len(&self) -> usize {
        self.base.min_len()
    }

    #[inline]
    fn max_len(&self) -> usize {
        self.base.max_len()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (
            EnumerateAdapter {
                base: left,
                offset: self.offset,
            },
            EnumerateAdapter {
                base: right,
                offset: self.offset + index,
            },
        )
    }
}

impl<I> Iterator for EnumerateAdapter<I>
where
    I: Iterator,
{
    type Item = (usize, I::Item);

    fn next(&mut self) -> Option<Self::Item> {
        self.base.next().map(|item| {
            let i = self.offset;
            self.offset += 1;
            (i, item)
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }

    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.base.count()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.base.nth(n).map(|item| {
            let i = self.offset + n;
            self.offset = i + 1;
            (i, item)
        })
    }

    fn fold<B, F>(self, init: B, mut func: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.base
            .fold((init, self.offset), |(accum, i), item| {
                (func(accum, (i, item)), i + 1)
            })
            .0
    }

    // TODO: try_fold, advance_by, when they land in stable or when `rayon`
    // gains a nightly feature
}

impl<I: ExactSizeIterator> ExactSizeIterator for EnumerateAdapter<I> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}

impl<I: DoubleEndedIterator + ExactSizeIterator> DoubleEndedIterator for EnumerateAdapter<I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.base
            .next_back()
            .map(|item| (self.offset + self.base.len(), item))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.base
            .nth_back(n)
            .map(|item| (self.offset + self.base.len(), item))
    }

    fn rfold<B, F>(self, init: B, mut func: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let reverse_offset = self.offset + self.base.len();

        self.base
            .rfold((init, reverse_offset), |(accum, offset), item| {
                (func(accum, (offset - 1, item)), offset - 1)
            })
            .0
    }

    // TODO: try_rfold, advance_back_by, when they land in stable or when `rayon`
    // gains a nightly feature
}
