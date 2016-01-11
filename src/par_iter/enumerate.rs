use super::{ParallelIterator, ParallelIteratorState, ParallelLen};

pub struct Enumerate<M> {
    base: M,
}

impl<M> Enumerate<M> {
    pub fn new(base: M) -> Enumerate<M> {
        Enumerate { base: base }
    }
}

impl<M> ParallelIterator for Enumerate<M>
    where M: ParallelIterator,
{
    type Item = (usize, M::Item);
    type Shared = EnumerateShared<M>;
    type State = EnumerateState<M>;

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (EnumerateShared { base: base_shared },
         EnumerateState { base: base_state, offset: 0 })
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

impl<M> ParallelIteratorState for EnumerateState<M>
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

    fn for_each<F>(self, shared: &Self::Shared, mut op: F)
        where F: FnMut(Self::Item)
    {
        let mut count = self.offset;
        self.base.for_each(&shared.base, |item| {
            op((count, item));
            count += 1;
        });
    }
}
