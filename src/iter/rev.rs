use super::internal::*;
use super::*;
use std::iter;

pub struct Rev<M: IndexedParallelIterator> {
    base: M,
}

/// Create a new `Rev` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<M>(base: M) -> Rev<M>
    where M: IndexedParallelIterator
{
    Rev { base: base }
}

impl<M> ParallelIterator for Rev<M>
    where M: IndexedParallelIterator
{
    type Item = M::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<M> BoundedParallelIterator for Rev<M>
    where M: IndexedParallelIterator
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
}

impl<M> ExactParallelIterator for Rev<M>
    where M: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M> IndexedParallelIterator for Rev<M>
    where M: IndexedParallelIterator
{
    fn with_producer<CB>(mut self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        let len = self.base.len();
        return self.base.with_producer(Callback {
                                           callback: callback,
                                           len: len,
                                       });

        struct Callback<CB> {
            callback: CB,
            len: usize,
        }

        impl<ITEM, CB> ProducerCallback<ITEM> for Callback<CB>
            where CB: ProducerCallback<ITEM>
        {
            type Output = CB::Output;
            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item = ITEM>
            {
                let producer = RevProducer {
                    base: base,
                    len: self.len,
                };
                self.callback.callback(producer)
            }
        }
    }
}

struct RevProducer<P> {
    base: P,
    len: usize,
}

impl<P> Producer for RevProducer<P>
    where P: Producer
{
    type Item = P::Item;
    type IntoIter = iter::Rev<P::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter().rev()
    }

    fn weighted(&self) -> bool {
        self.base.weighted()
    }

    fn cost(&mut self, items: usize) -> f64 {
        self.base.cost(items)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(self.len - index);
        (RevProducer {
             base: right,
             len: index,
         },
         RevProducer {
             base: left,
             len: self.len - index,
         })
    }
}
