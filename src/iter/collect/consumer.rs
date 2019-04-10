use super::super::plumbing::*;
use std::ptr;
use std::slice;

pub struct CollectConsumer<'c, T: Send + 'c> {
    /// Tracks how many items we successfully wrote. Used to guarantee
    /// safety in the face of panics or buggy parallel iterators.
    writes: usize,

    /// A slice covering the target memory, not yet initialized!
    target: &'c mut [T],
}

pub struct CollectFolder<'c, T: Send + 'c> {
    writes: usize,

    /// An iterator over the *uninitialized* target memory.
    target: slice::IterMut<'c, T>,
}

impl<'c, T: Send + 'c> CollectConsumer<'c, T> {
    /// The target memory is considered uninitialized, and will be
    /// overwritten without dropping anything.
    pub fn new(target: &'c mut [T]) -> CollectConsumer<'c, T> {
        CollectConsumer {
            writes: 0,
            target: target,
        }
    }
}

impl<'c, T: Send + 'c> Consumer<T> for CollectConsumer<'c, T> {
    type Folder = CollectFolder<'c, T>;
    type Reducer = CollectReducer;
    type Result = usize;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        // instances Read in the fields from `self` and then
        // forget `self`, since it has been legitimately consumed
        // (and not dropped during unwinding).
        let CollectConsumer { writes, target } = self;

        // Produce new consumers. Normal slicing ensures that the
        // memory range given to each consumer is disjoint.
        let (left, right) = target.split_at_mut(index);
        (
            CollectConsumer { writes: writes, target: left },
            CollectConsumer::new(right),
            CollectReducer,
        )
    }

    fn into_folder(self) -> CollectFolder<'c, T> {
        CollectFolder {
            writes: self.writes,
            target: self.target.into_iter(),
        }
    }

    fn full(&self) -> bool {
        false
    }
}

impl<'c, T: Send + 'c> Folder<T> for CollectFolder<'c, T> {
    type Result = usize;

    fn consume(mut self, item: T) -> CollectFolder<'c, T> {
        // Compute target pointer and write to it. Safe because the iterator
        // does all the bounds checking; we're only avoiding the target drop.
        let head = self
            .target
            .next()
            .expect("too many values pushed to consumer");
        unsafe {
            ptr::write(head, item);
        }

        self.writes += 1;
        self
    }

    fn complete(self) -> usize {
        assert!(self.target.len() == 0, "too few values pushed to consumer");

        // track total values written
        self.writes
    }

    fn full(&self) -> bool {
        false
    }
}

/// Pretend to be unindexed for `special_collect_into_vec`,
/// but we should never actually get used that way...
impl<'c, T: Send + 'c> UnindexedConsumer<T> for CollectConsumer<'c, T> {
    fn split_off_left(&self) -> Self {
        unreachable!("CollectConsumer must be indexed!")
    }

    fn to_reducer(&self) -> Self::Reducer {
        CollectReducer
    }
}

pub struct CollectReducer;

impl Reducer<usize> for CollectReducer {
    fn reduce(self, left: usize, right: usize) -> usize {
        left + right
    }
}
