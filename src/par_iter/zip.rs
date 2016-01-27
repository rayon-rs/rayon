use super::*;
use super::internal::*;
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

unsafe impl<A,B> BoundedParallelIterator for ZipIter<A,B>
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

unsafe impl<A,B> ExactParallelIterator for ZipIter<A,B>
    where A: IndexedParallelIterator, B: IndexedParallelIterator
{
    fn len(&mut self) -> usize {
        min(self.a.len(), self.b.len())
    }
}

impl<A,B> IndexedParallelIterator for ZipIter<A,B>
    where A: IndexedParallelIterator, B: IndexedParallelIterator
{
    fn with_producer<WP>(self, wp: WP) -> WP::Output
        where WP: ProducerContinuation<Self::Item>
    {
        return self.a.with_producer(ContinuationA {
            wp: wp,
            b: self.b,
        });

        struct ContinuationA<WP, B> {
            wp: WP,
            b: B
        }

        impl<WP, A_ITEM, B> ProducerContinuation<A_ITEM> for ContinuationA<WP, B>
            where B: IndexedParallelIterator,
                  WP: ProducerContinuation<(A_ITEM, B::Item)>,
        {
            type Output = WP::Output;

            fn with_producer<P>(self, a_producer: P, a_shared: P::Shared) -> Self::Output
                where P: Producer<Item=A_ITEM>
            {
                return self.b.with_producer(ContinuationB {
                    a_producer: a_producer,
                    a_shared: a_shared,
                    wp: self.wp
                });
            }
        }

        struct ContinuationB<WP, A> where A: Producer {
            a_producer: A,
            a_shared: A::Shared,
            wp: WP
        }

        impl<WP, A, B_ITEM> ProducerContinuation<B_ITEM> for ContinuationB<WP, A>
            where A: Producer,
                  WP: ProducerContinuation<(A::Item, B_ITEM)>,
        {
            type Output = WP::Output;

            fn with_producer<P>(self, b_producer: P, b_shared: P::Shared) -> Self::Output
                where P: Producer<Item=B_ITEM>
            {
                self.wp.with_producer(ZipProducer { p: self.a_producer, q: b_producer },
                                      (self.a_shared, b_shared))
            }
        }

    }
}

///////////////////////////////////////////////////////////////////////////

pub struct ZipProducer<P: Producer, Q: Producer> {
    p: P,
    q: Q,
}

impl<P: Producer, Q: Producer> Producer for ZipProducer<P, Q>
{
    type Item = (P::Item, Q::Item);
    type Shared = (P::Shared, Q::Shared);

    fn cost(&mut self, shared: &Self::Shared, len: usize) -> f64 {
        // Rather unclear that this should be `+`. It might be that max is better?
        self.p.cost(&shared.0, len) + self.q.cost(&shared.1, len)
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (p_left, p_right) = self.p.split_at(index);
        let (q_left, q_right) = self.q.split_at(index);
        (ZipProducer { p: p_left, q: q_left }, ZipProducer { p: p_right, q: q_right })
    }

    unsafe fn produce(&mut self, shared: &(P::Shared, Q::Shared)) -> (P::Item, Q::Item) {
        let p = self.p.produce(&shared.0);
        let q = self.q.produce(&shared.1);
        (p, q)
    }
}
