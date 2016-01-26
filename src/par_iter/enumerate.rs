use super::*;
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
    where M: PullParallelIterator,
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
    where M: PullParallelIterator,
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }
}

unsafe impl<M> ExactParallelIterator for Enumerate<M>
    where M: PullParallelIterator,
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M> PullParallelIterator for Enumerate<M>
    where M: PullParallelIterator,
{
    type Producer = EnumerateProducer<M::Producer>;

    fn into_producer(self) -> (Self::Producer, <Self::Producer as Producer>::Shared) {
        let (base, shared) = self.base.into_producer();
        (EnumerateProducer { base: base, offset: 0 }, shared)
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

impl<M> Producer for EnumerateProducer<M>
    where M: Producer
{
    type Item = (usize, M::Item);
    type Shared = M::Shared;

    unsafe fn cost(&mut self, shared: &Self::Shared, items: usize) -> f64 {
        self.base.cost(shared, items) // enumerating is basically free
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (EnumerateProducer { base: left, offset: self.offset },
         EnumerateProducer { base: right, offset: self.offset + index })
    }

    unsafe fn produce(&mut self, shared: &Self::Shared) -> (usize, M::Item) {
        let item = self.base.produce(shared);
        let index = self.offset;
        self.offset += 1;
        (index, item)
    }
}
