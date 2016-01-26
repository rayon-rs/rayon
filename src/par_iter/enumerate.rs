use super::{ParallelIterator, BoundedParallelIterator, ExactParallelIterator};
use super::len::ParallelLen;
use super::state::*;

pub struct Enumerate<M> {
    base: M,
}

impl<M> Enumerate<M> {
    pub fn new(base: M) -> Enumerate<M> {
        Enumerate { base: base }
    }
}

impl<M> ParallelIterator for Enumerate<M>
    where M: ExactParallelIterator,
{
    type Item = (usize, M::Item);
    type Shared = EnumerateShared<M>;
    type State = EnumerateState<M>;

    fn drive<C: Consumer<Item=Self::Item>>(self, _consumer: C) -> C::Result {
        unimplemented!()
    }

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (EnumerateShared { base: base_shared },
         EnumerateState { base: base_state, offset: 0 })
    }
}

unsafe impl<M> BoundedParallelIterator for Enumerate<M>
    where M: ExactParallelIterator,
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }
}

unsafe impl<M> ExactParallelIterator for Enumerate<M>
    where M: ExactParallelIterator,
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

pub struct EnumerateState<M>
    where M: ParallelIterator
{
    base: M::State,
    offset: usize,
}

pub struct EnumerateShared<M>
    where M: ParallelIterator
{
    base: M::Shared,
}

unsafe impl<M> ParallelIteratorState for EnumerateState<M>
    where M: ParallelIterator,
{
    type Item = (usize, M::Item);
    type Shared = EnumerateShared<M>;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen {
        self.base.len(&shared.base)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (EnumerateState { base: left, offset: self.offset },
         EnumerateState { base: right, offset: self.offset + index })
    }

    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item> {
        self.base.next(&shared.base)
                 .map(|base| {
                     let index = self.offset;
                     self.offset += 1;
                     (index, base)
                 })
    }
}

///////////////////////////////////////////////////////////////////////////
// Producer implementation

pub struct EnumerateProducer<M>
    where M: Producer,
{
    base: M,
    offset: usize,
}

pub struct EnumerateProducerShared<M>
    where M: Producer,
{
    base: M::Shared,
}

impl<M> Producer for EnumerateProducer<M>
    where M: Producer
{
    type Item = (usize, M::Item);
    type Shared = EnumerateProducerShared<M>;

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (EnumerateProducer { base: left, offset: self.offset },
         EnumerateProducer { base: right, offset: self.offset + index })
    }

    unsafe fn produce(&mut self,
                      shared: &EnumerateProducerShared<M>)
                      -> (usize, M::Item)
    {
        let item = self.base.produce(&shared.base);
        let index = self.offset;
        self.offset += 1;
        (index, item)
    }
}
