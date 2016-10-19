use super::*;
use super::internal::*;
use std::iter;
use std::ops::RangeFrom;

pub struct Enumerate<M> {
    base: M,
}

impl<M> Enumerate<M> {
    pub fn new(base: M) -> Enumerate<M> {
        Enumerate { base: base }
    }
}

impl<M> ParallelIterator for Enumerate<M>
    where M: IndexedParallelIterator,
{
    type Item = (usize, M::Item);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<M> BoundedParallelIterator for Enumerate<M>
    where M: IndexedParallelIterator,
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
}

impl<M> ExactParallelIterator for Enumerate<M>
    where M: IndexedParallelIterator,
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M> IndexedParallelIterator for Enumerate<M>
    where M: IndexedParallelIterator,
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback { callback: callback });

        struct Callback<CB> {
            callback: CB,
        }

        impl<ITEM, CB> ProducerCallback<ITEM> for Callback<CB>
            where CB: ProducerCallback<(usize, ITEM)>
        {
            type Output = CB::Output;
            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item=ITEM>
            {
                let producer = EnumerateProducer { base: base,
                                                   offset: 0 };
                self.callback.callback(producer)
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// Producer implementation

pub struct EnumerateProducer<P> {
    base: P,
    offset: usize,
}

impl<P> Producer for EnumerateProducer<P>
    where P: Producer
{
    fn weighted(&self) -> bool {
        self.base.weighted()
    }

    fn cost(&mut self, items: usize) -> f64 {
        self.base.cost(items) // enumerating is basically free
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (EnumerateProducer { base: left,
                             offset: self.offset },
         EnumerateProducer { base: right,
                             offset: self.offset + index })
    }
}

impl<P> IntoIterator for EnumerateProducer<P> where P: Producer {
    type Item = (usize, P::Item);
    type IntoIter = iter::Zip<RangeFrom<usize>, P::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        (self.offset..).zip(self.base)
    }
}
