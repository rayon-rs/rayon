use super::internal::*;
use super::*;
use std::cmp;
use std::iter;
use std::usize;

/// `Interleave` is an iterator that interleaves elements of iterators
/// `i` and `j` in one continuous iterator. This struct is created by
/// the [`interleave()`] method on [`IndexedParallelIterator`]
///
/// [`interleave()`]: trait.IndexedParallelIterator.html#method.interleave
/// [`IndexedParallelIterator`]: trait.IndexedParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Interleave<I, J>
    where I: IndexedParallelIterator,
          J: IndexedParallelIterator<Item = I::Item>
{
    i: I,
    j: J,
    flag: bool,
}

/// Create a new `Interleave` iterator
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I, J>(i: I, j: J) -> Interleave<I, J>
    where I: IndexedParallelIterator,
          J: IndexedParallelIterator<Item = I::Item>
{
    let flag = false;
    Interleave { i, j, flag }
}

impl<I, J> ParallelIterator for Interleave<I, J>
    where I: IndexedParallelIterator,
          J: IndexedParallelIterator<Item = I::Item>
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: Consumer<I::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I, J> IndexedParallelIterator for Interleave<I, J>
    where I: IndexedParallelIterator,
          J: IndexedParallelIterator<Item = I::Item>,
{
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        self.i.len() + self.j.len()
    }

    fn with_producer<CB>(mut self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        let (i_len, j_len) = (self.i.len(), self.j.len());
        return self.i.with_producer(CallbackI {
            callback: callback,
            i_len: i_len,
            j_len: j_len,
            j: self.j
        });

        struct CallbackI<CB, J> {
            callback: CB,
            i_len: usize,
            j_len: usize,
            j: J
        }

        impl<CB, J> ProducerCallback<J::Item> for CallbackI<CB, J>
            where J: IndexedParallelIterator,
                  CB: ProducerCallback<J::Item>
        {
            type Output = CB::Output;

            fn callback<I>(self, i_producer: I) -> Self::Output
                where I: Producer<Item = J::Item>
            {
                self.j.with_producer(CallbackJ {
                    i_producer: i_producer,
                    i_len: self.i_len,
                    j_len: self.j_len,
                    callback: self.callback
                })
            }
        }

        struct CallbackJ<CB, I> {
            callback: CB,
            i_len: usize,
            j_len: usize,
            i_producer: I
        }

        impl<CB, I> ProducerCallback<I::Item> for CallbackJ<CB, I>
            where I: Producer,
                  CB: ProducerCallback<I::Item>
        {
            type Output = CB::Output;

            fn callback<J>(self, j_producer: J) -> Self::Output
                where J: Producer<Item = I::Item>
            {
                let producer = InterleaveProducer::new(self.i_producer, j_producer, self.i_len, self.j_len, false);
                self.callback.callback(producer)
            }
        }
    }
}

pub struct InterleaveProducer<I, J>
    where I: Producer,
          J: Producer<Item = I::Item>
{
    i: I,
    j: J,
    i_len: usize,
    j_len: usize,
    flag: bool,
}

impl<I, J> InterleaveProducer<I, J>
    where I: Producer,
          J: Producer<Item = I::Item>
{
    fn new(i: I, j: J, i_len: usize, j_len: usize, flag: bool) -> InterleaveProducer<I, J> {
        InterleaveProducer { i, j, i_len, j_len, flag }
    }
}

impl<I, J> Producer for InterleaveProducer<I, J>
    where I: Producer,
          J: Producer<Item = I::Item>
{
    type Item = I::Item;
    type IntoIter = InterleaveSeq<I::IntoIter, J::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        InterleaveSeq::new(self.i.into_iter(), self.j.into_iter())
    }

    fn min_len(&self) -> usize {
        cmp::max(self.i.min_len(), self.j.min_len())
    }

    fn max_len(&self) -> usize {
        cmp::min(self.i.max_len(), self.j.max_len())
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        // Switch on state
        let even = index%2 == 0;
        let idx = index >> 1;

        // We know 0 < index <= self.i_len + self.j_len
        // Find a, b satisfying the following conditions:
        // (1) 0 < a <= self.i_len
        // (2) 0 < b <= self.j_len
        // (3) a + b == index
        // assert!(index <= self.i_len + self.j_len);

        let odd_offset = |flag| if flag { 0 } else { 1 };

        // desired split
        let (i_idx, j_idx) = (idx + odd_offset(even || self.flag),
                              idx + odd_offset(even || !self.flag));

        let (i_split, j_split) = if self.i_len >= i_idx && self.j_len >= j_idx {
            (i_idx, j_idx)
        } else if self.i_len >= i_idx {
            // j too short
            (index - self.j_len, self.j_len)
        } else {
            // i too short
            (self.i_len, index - self.i_len)
        };

        let trailing_flag = even == self.flag;
        let (i_left, i_right) = self.i.split_at(i_split);
        let (j_left, j_right) = self.j.split_at(j_split);

        (InterleaveProducer::new(
            i_left,
            j_left,
            i_split,
            j_split,
            self.flag
        ), InterleaveProducer::new(
            i_right,
            j_right,
            self.i_len - i_split,
            self.j_len - j_split,
            trailing_flag
        ))
    }
}


/// Wrapper for Interleave to implement DoubleEndedIterator and
/// ExactSizeIterator
pub struct InterleaveSeq<I, J> {
    i: I,
    j: J,
    flag: bool
}

impl<I, J> InterleaveSeq<I, J> {
    fn new(i: I, j: J) -> InterleaveSeq<I, J>
        where I: DoubleEndedIterator + ExactSizeIterator,
              J: DoubleEndedIterator<Item = I::Item> + ExactSizeIterator<Item = I::Item>
    {
        InterleaveSeq { i: i, j: j, flag: false }
    }
}

impl<I, J> Iterator for InterleaveSeq<I, J>
    where I: Iterator,
          J: Iterator<Item = I::Item>
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.flag = !self.flag;
        if self.flag {
            match self.i.next() {
                None => self.j.next(),
                r => r
            }
        } else {
            match self.j.next() {
                None => self.i.next(),
                r => r
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (ih, jh) = (self.i.size_hint(), self.j.size_hint());
        let min = ih.0.checked_add(jh.0).unwrap_or(usize::MAX);
        let max = match (ih.1, jh.1) {
            (Some(x), Some(y)) => x.checked_add(y),
            _=> None
        };
        (min, max)
    }
}

// The implementation for DoubleEndedIterator requires
// ExactSizeIterator to provide `next_back()`. The last element will
// come from the iterator that runs out last (ie has the most elements
// in it). If the iterators have the same number of elements, then the
// last iterator will provide the last element.
impl<I, J> DoubleEndedIterator for InterleaveSeq<I, J>
    where I: DoubleEndedIterator + ExactSizeIterator,
          J: DoubleEndedIterator<Item = I::Item> + ExactSizeIterator<Item = I::Item>
{
    #[inline]
    fn next_back(&mut self) -> Option<I::Item> {
        if self.i.len() <= self.j.len() {
            self.j.next_back()
        } else {
            self.i.next_back()
        }
    }
}

impl<I, J> ExactSizeIterator for InterleaveSeq<I, J>
    where I: ExactSizeIterator,
          J: ExactSizeIterator<Item = I::Item>
{
    #[inline]
    fn len(&self) -> usize {
        self.i.len() + self.j.len()
    }
}
