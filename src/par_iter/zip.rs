use super::*;
use super::state::*;
use std::cmp::min;

pub struct ZipIter<A: PullParallelIterator, B: PullParallelIterator> {
    a: A,
    b: B,
}

impl<A: PullParallelIterator, B: PullParallelIterator> ZipIter<A, B> {
    pub fn new(a: A, b: B) -> ZipIter<A, B> {
        ZipIter { a: a, b: b }
    }
}

impl<A, B> ParallelIterator for ZipIter<A, B>
    where A: PullParallelIterator, B: PullParallelIterator
{
    type Item = (A::Item, B::Item);

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        bridge(self, consumer, &shared)
    }
}

unsafe impl<A,B> BoundedParallelIterator for ZipIter<A,B>
    where A: PullParallelIterator, B: PullParallelIterator
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }
}

unsafe impl<A,B> ExactParallelIterator for ZipIter<A,B>
    where A: PullParallelIterator, B: PullParallelIterator
{
    fn len(&mut self) -> usize {
        min(self.a.len(), self.b.len())
    }
}

impl<A,B> PullParallelIterator for ZipIter<A,B>
    where A: PullParallelIterator, B: PullParallelIterator
{
    type Producer = ZipProducer<A::Producer, B::Producer>;

    fn into_producer(self) -> (Self::Producer, <Self::Producer as Producer>::Shared) {
        let (a_producer, a_shared) = self.a.into_producer();
        let (b_producer, b_shared) = self.b.into_producer();
        (ZipProducer { p: a_producer, q: b_producer }, (a_shared, b_shared))
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
