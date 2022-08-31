use super::plumbing::*;
use super::*;

/// `Arrays` is an iterator that groups elements of an underlying iterator.
///
/// This struct is created by the [`arrays()`] method on [`IndexedParallelIterator`]
///
/// [`arrays()`]: trait.IndexedParallelIterator.html#method.arrays
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct Arrays<I, const N: usize>
where
    I: IndexedParallelIterator,
{
    iter: I,
}

impl<I, const N: usize> Arrays<I, N>
where
    I: IndexedParallelIterator,
{
    /// Creates a new `Arrays` iterator
    pub(super) fn new(iter: I) -> Self {
        Arrays { iter }
    }
}

impl<I, const N: usize> ParallelIterator for Arrays<I, N>
where
    I: IndexedParallelIterator,
{
    type Item = [I::Item; N];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I, const N: usize> IndexedParallelIterator for Arrays<I, N>
where
    I: IndexedParallelIterator,
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.iter.len() / N
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        let len = self.iter.len();
        return self.iter.with_producer(Callback { len, callback });

        struct Callback<CB, const N: usize> {
            len: usize,
            callback: CB,
        }

        impl<T, CB, const N: usize> ProducerCallback<T> for Callback<CB, N>
        where
            CB: ProducerCallback<[T; N]>,
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> CB::Output
            where
                P: Producer<Item = T>,
            {
                self.callback.callback(ArrayProducer {
                    len: self.len,
                    base,
                })
            }
        }
    }
}

struct ArrayProducer<P, const N: usize>
where
    P: Producer,
{
    len: usize,
    base: P,
}

impl<P, const N: usize> Producer for ArrayProducer<P, N>
where
    P: Producer,
{
    type Item = [P::Item; N];
    type IntoIter = ArraySeq<P, N>;

    fn into_iter(self) -> Self::IntoIter {
        // TODO: we're ignoring any remainder -- should we no-op consume it?
        let remainder = self.len % N;
        let len = self.len - remainder;
        let inner = (len > 0).then(|| self.base.split_at(len).0);
        ArraySeq { len, inner }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * N;
        let (left, right) = self.base.split_at(elem_index);
        (
            ArrayProducer {
                len: elem_index,
                base: left,
            },
            ArrayProducer {
                len: self.len - elem_index,
                base: right,
            },
        )
    }

    fn min_len(&self) -> usize {
        self.base.min_len() / N
    }

    fn max_len(&self) -> usize {
        self.base.max_len() / N
    }
}

struct ArraySeq<P, const N: usize> {
    len: usize,
    inner: Option<P>,
}

impl<P, const N: usize> Iterator for ArraySeq<P, N>
where
    P: Producer,
{
    type Item = [P::Item; N];

    fn next(&mut self) -> Option<Self::Item> {
        let mut producer = self.inner.take()?;
        debug_assert!(self.len > 0 && self.len % N == 0);
        if self.len > N {
            let (left, right) = producer.split_at(N);
            producer = left;
            self.inner = Some(right);
            self.len -= N;
        } else {
            self.len = 0;
        }
        Some(collect_array(producer.into_iter()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<P, const N: usize> ExactSizeIterator for ArraySeq<P, N>
where
    P: Producer,
{
    #[inline]
    fn len(&self) -> usize {
        self.len / N
    }
}

impl<P, const N: usize> DoubleEndedIterator for ArraySeq<P, N>
where
    P: Producer,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let mut producer = self.inner.take()?;
        debug_assert!(self.len > 0 && self.len % N == 0);
        if self.len > N {
            let (left, right) = producer.split_at(self.len - N);
            producer = right;
            self.inner = Some(left);
            self.len -= N;
        } else {
            self.len = 0;
        }
        Some(collect_array(producer.into_iter()))
    }
}

fn collect_array<T, const N: usize>(mut iter: impl ExactSizeIterator<Item = T>) -> [T; N] {
    // TODO(MSRV-1.55): consider `[(); N].map(...)`
    // TODO(MSRV-1.63): consider `std::array::from_fn`

    use std::mem::MaybeUninit;

    // TODO(MSRV): use `MaybeUninit::uninit_array` when/if it's stabilized.
    // SAFETY: We can assume "init" when moving uninit wrappers inward.
    let mut array: [MaybeUninit<T>; N] =
        unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() };

    debug_assert_eq!(iter.len(), N);
    for i in 0..N {
        let item = iter.next().expect("should have N items");
        array[i] = MaybeUninit::new(item);
    }
    debug_assert!(iter.next().is_none());

    // TODO(MSRV): use `MaybeUninit::array_assume_init` when/if it's stabilized.
    // SAFETY: We've initialized all N items in the array, so we can cast and "move" it.
    unsafe { (&array as *const [MaybeUninit<T>; N] as *const [T; N]).read() }
}
