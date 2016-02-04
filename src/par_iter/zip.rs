use super::*;
use super::internal::*;
use super::util::PhantomType;
use std::cmp::min;

pub struct ZipIter<A: IndexedParallelIterator, B: IndexedParallelIterator> {
    a: A,
    b: B,
}

impl<A: IndexedParallelIterator, B: IndexedParallelIterator> ZipIter<A, B> {
    pub fn new(a: A, b: B) -> ZipIter<A, B> {
        ZipIter { a: a, b: b }
    }
}

impl<A, B> ParallelIterator for ZipIter<A, B>
    where A: IndexedParallelIterator, B: IndexedParallelIterator
{
    type Item = (A::Item, B::Item);

    fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                      consumer: C,
                                                                      shared: &'c C::Shared)
                                                                      -> C::Result {
        bridge(self, consumer, &shared)
    }
}

impl<A,B> BoundedParallelIterator for ZipIter<A,B>
    where A: IndexedParallelIterator, B: IndexedParallelIterator
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        bridge(self, consumer, &shared)
    }
}

impl<A,B> ExactParallelIterator for ZipIter<A,B>
    where A: IndexedParallelIterator, B: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        min(self.a.len(), self.b.len())
    }
}

impl<A,B> IndexedParallelIterator for ZipIter<A,B>
    where A: IndexedParallelIterator, B: IndexedParallelIterator
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.a.with_producer(CallbackA {
            callback: callback,
            b: self.b,
        });

        struct CallbackA<CB, B> {
            callback: CB,
            b: B
        }

        impl<CB, A_ITEM, B> ProducerCallback<A_ITEM> for CallbackA<CB, B>
            where B: IndexedParallelIterator,
                  CB: ProducerCallback<(A_ITEM, B::Item)>,
        {
            type Output = CB::Output;

            fn callback<'a, A>(self, a_producer: A, a_shared: &'a A::Shared) -> Self::Output
                where A: Producer<'a, Item=A_ITEM>
            {
                return self.b.with_producer(CallbackB {
                    a_producer: a_producer,
                    a_shared: a_shared,
                    callback: self.callback,
                });
            }
        }

        struct CallbackB<'p, CB, A> where A: Producer<'p> {
            a_producer: A,
            a_shared: &'p A::Shared,
            callback: CB
        }

        impl<'a, CB, A, B_ITEM> ProducerCallback<B_ITEM> for CallbackB<'a, CB, A>
            where A: Producer<'a>,
                  CB: ProducerCallback<(A::Item, B_ITEM)>,
        {
            type Output = CB::Output;

            fn callback<'b, B>(self, b_producer: B, b_shared: &'b B::Shared) -> Self::Output
                where B: Producer<'b, Item=B_ITEM>
            {
                self.callback.callback(ZipProducer { p: self.a_producer, q: b_producer,
                                                     phantoms: PhantomType::new() },
                                       &(self.a_shared, b_shared))
            }
        }

    }
}

///////////////////////////////////////////////////////////////////////////

pub struct ZipProducer<'a, 'b, A: Producer<'a>, B: Producer<'b>> {
    p: A,
    q: B,
    phantoms: PhantomType<(&'a (), &'b ())>,
}

impl<'z, 'a, 'b, A: Producer<'a>, B: Producer<'b>> Producer<'z> for ZipProducer<'a, 'b, A, B>
    where 'a: 'z, 'b: 'z
{
    type Item = (A::Item, B::Item);
    type Shared = (&'a A::Shared, &'b B::Shared);

    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64 {
        // Rather unclear that this should be `+`. It might be that max is better?
        self.p.cost(&shared.0, len) + self.q.cost(&shared.1, len)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (p_left, p_right) = self.p.split_at(index);
        let (q_left, q_right) = self.q.split_at(index);
        (ZipProducer { p: p_left, q: q_left, phantoms: PhantomType::new() },
         ZipProducer { p: p_right, q: q_right, phantoms: PhantomType::new() })
    }

    fn produce(&mut self, shared: &(&A::Shared, &B::Shared)) -> (A::Item, B::Item) {
        let p = self.p.produce(shared.0);
        let q = self.q.produce(shared.1);
        (p, q)
    }
}
