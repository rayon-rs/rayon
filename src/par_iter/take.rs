use super::*;
use super::internal::*;

pub struct Take<M> {
    base: M,
    n: usize,
}

impl<M> Take<M> {
    pub fn new(base: M, n: usize) -> Take<M> {
        Take { base: base,
               n: n }
    }
}

impl<M> ParallelIterator for Take<M>
    where M: IndexedParallelIterator
{
    type Item = M::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }
}

impl<M> ExactParallelIterator for Take<M>
    where M: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        let base_len = self.base.len();
        if self.n > base_len {
            self.n = base_len
        }
        self.n
    }
}

impl<M> BoundedParallelIterator for Take<M>
    where M: IndexedParallelIterator
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
}

impl<M> IndexedParallelIterator for Take<M>
    where M: IndexedParallelIterator
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback { callback: callback,
                                                  n: self.n});

        struct Callback<CB> {
            callback: CB,
            n: usize,
        }

        impl<ITEM, CB> ProducerCallback<ITEM> for Callback<CB>
            where CB: ProducerCallback<ITEM>
        {
            type Output = CB::Output;
            fn callback<P>(self, base: P) -> CB::Output
                where P: Producer<Item=ITEM>
            {
                let (producer, _) = base.split_at(self.n);
                self.callback.callback(producer)
            }
        }
    }
}
