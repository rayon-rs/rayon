use super::{ParallelIterator, BoundedParallelIterator, ExactParallelIterator};
use super::len::ParallelLen;
use super::state::ParallelIteratorState;
use std::cmp::min;

pub struct ZipIter<A: ExactParallelIterator, B: ExactParallelIterator> {
    a: A,
    b: B,
}

impl<A: ExactParallelIterator, B: ExactParallelIterator> ZipIter<A, B> {
    pub fn new(a: A, b: B) -> ZipIter<A, B> {
        ZipIter { a: a, b: b }
    }
}

impl<A, B> ParallelIterator for ZipIter<A, B>
    where A: ExactParallelIterator, B: ExactParallelIterator
{
    type Item = (A::Item, B::Item);
    type Shared = ZipShared<A,B>;
    type State = ZipState<A,B>;

    fn state(self) -> (Self::Shared, Self::State) {
        let (a_shared, a_state) = self.a.state();
        let (b_shared, b_state) = self.b.state();
        (ZipShared { a: a_shared, b: b_shared }, ZipState { a: a_state, b: b_state })
    }
}

unsafe impl<A,B> BoundedParallelIterator for ZipIter<A,B>
    where A: ExactParallelIterator, B: ExactParallelIterator
{}

unsafe impl<A,B> ExactParallelIterator for ZipIter<A,B>
    where A: ExactParallelIterator, B: ExactParallelIterator
{}

pub struct ZipShared<A: ExactParallelIterator, B: ExactParallelIterator> {
    a: A::Shared,
    b: B::Shared,
}

pub struct ZipState<A: ExactParallelIterator, B: ExactParallelIterator> {
    a: A::State,
    b: B::State,
}

unsafe impl<A, B> ParallelIteratorState for ZipState<A,B>
    where A: ExactParallelIterator, B: ExactParallelIterator
{
    type Item = (A::Item, B::Item);
    type Shared = ZipShared<A, B>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        let a_len = self.a.len(&shared.a);
        let b_len = self.b.len(&shared.b);

        // Because A and B are exact parallel iterators, sparse should
        // always be false.
        assert!(!a_len.sparse, "A is an exact parallel iterator, so parse should be false");
        assert!(!b_len.sparse, "B is an exact parallel iterator, so parse should be false");

        ParallelLen {
            maximal_len: min(a_len.maximal_len, b_len.maximal_len),
            cost: a_len.cost + b_len.cost,
            sparse: false,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (a_left, a_right) = self.a.split_at(index);
        let (b_left, b_right) = self.b.split_at(index);
        (ZipState { a: a_left, b: b_left }, ZipState { a: a_right, b: b_right })
    }

    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item> {
        let a_next = self.a.next(&shared.a);
        let b_next = self.b.next(&shared.b);
        if let Some(a_item) = a_next {
            if let Some(b_item) = b_next {
                return Some((a_item, b_item));
            }
        }
        None
    }
}
