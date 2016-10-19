use super::*;
use super::internal::*;
use std::cmp::min;
use std::iter;

pub struct ChainIter<A, B>
    where A: ParallelIterator, B: ParallelIterator<Item=A::Item>
{
    a: A,
    b: B,
}

impl<A, B> ChainIter<A, B>
    where A: ParallelIterator, B: ParallelIterator<Item=A::Item>
{
    pub fn new(a: A, b: B) -> ChainIter<A, B> {
        ChainIter { a: a, b: b }
    }
}

impl<A, B> ParallelIterator for ChainIter<A, B>
    where A: ParallelIterator, B: ParallelIterator<Item=A::Item>
{
    type Item = A::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let a = self.a.drive_unindexed(consumer.split_off());
        let b = self.b.drive_unindexed(consumer.split_off());
        consumer.to_reducer().reduce(a, b)
    }
}

impl<A, B> BoundedParallelIterator for ChainIter<A, B>
    where A: BoundedParallelIterator, B: BoundedParallelIterator<Item=A::Item>
{
    fn upper_bound(&mut self) -> usize {
        self.a.upper_bound() + self.b.upper_bound()
    }

    fn drive<C>(mut self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        let (left, right, reducer) = consumer.split_at(self.a.upper_bound());
        let a = self.a.drive(left);
        let b = self.b.drive(right);
        reducer.reduce(a, b)
    }
}

impl<A, B> ExactParallelIterator for ChainIter<A, B>
    where A: ExactParallelIterator, B: ExactParallelIterator<Item=A::Item>
{
    fn len(&mut self) -> usize {
        self.a.len() + self.b.len()
    }
}

impl<A, B> IndexedParallelIterator for ChainIter<A, B>
    where A: IndexedParallelIterator, B: IndexedParallelIterator<Item=A::Item>
{
    fn with_producer<CB>(mut self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        let a_len = self.a.len();
        return self.a.with_producer(CallbackA {
            callback: callback,
            a_len: a_len,
            b: self.b,
        });

        struct CallbackA<CB, B> {
            callback: CB,
            a_len: usize,
            b: B,
        }

        impl<CB, B> ProducerCallback<B::Item> for CallbackA<CB, B>
            where B: IndexedParallelIterator,
                  CB: ProducerCallback<B::Item>,
        {
            type Output = CB::Output;

            fn callback<A>(self, a_producer: A) -> Self::Output
                where A: Producer<Item=B::Item>
            {
                return self.b.with_producer(CallbackB {
                    callback: self.callback,
                    a_len: self.a_len,
                    a_producer: a_producer,
                });
            }
        }

        struct CallbackB<CB, A> {
            callback: CB,
            a_len: usize,
            a_producer: A,
        }

        impl<CB, A> ProducerCallback<A::Item> for CallbackB<CB, A>
            where A: Producer,
                  CB: ProducerCallback<A::Item>,
        {
            type Output = CB::Output;

            fn callback<B>(self, b_producer: B) -> Self::Output
                where B: Producer<Item=A::Item>
            {
                let producer = ChainProducer::new(self.a_len,
                                                  self.a_producer,
                                                  b_producer);
                self.callback.callback(producer)
            }
        }

    }
}

///////////////////////////////////////////////////////////////////////////

pub struct ChainProducer<A, B>
    where A: Producer, B: Producer<Item=A::Item>
{
    a_len: usize,
    a: A,
    b: B,
}

impl<A, B> ChainProducer<A, B>
    where A: Producer, B: Producer<Item=A::Item>
{
    fn new(a_len: usize, a: A, b: B) -> Self {
        ChainProducer { a_len: a_len, a: a, b: b }
    }
}

impl<A, B> Producer for ChainProducer<A, B>
    where A: Producer, B: Producer<Item=A::Item>
{
    fn weighted(&self) -> bool {
        self.a.weighted() || self.b.weighted()
    }

    fn cost(&mut self, len: usize) -> f64 {
        let a_len = min(self.a_len, len);
        let b_len = len - a_len;
        self.a.cost(a_len) + self.b.cost(b_len)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        if index <= self.a_len {
            let a_rem = self.a_len - index;
            let (a_left, a_right) = self.a.split_at(index);
            let (b_left, b_right) = self.b.split_at(0);
            (ChainProducer::new(index, a_left, b_left),
             ChainProducer::new(a_rem, a_right, b_right))
        } else {
            let (a_left, a_right) = self.a.split_at(self.a_len);
            let (b_left, b_right) = self.b.split_at(index - self.a_len);
            (ChainProducer::new(self.a_len, a_left, b_left),
             ChainProducer::new(0, a_right, b_right))
        }
    }
}

impl<A, B> IntoIterator for ChainProducer<A, B>
    where A: Producer, B: Producer<Item=A::Item>
{
    type Item = A::Item;
    type IntoIter = iter::Chain<A::IntoIter, B::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        self.a.into_iter().chain(self.b)
    }
}
