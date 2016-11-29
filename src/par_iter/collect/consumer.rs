use super::super::len::*;
use super::super::internal::*;
use super::super::noop::*;
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct CollectConsumer<'c, ITEM: Send + 'c> {
    /// Tracks how many items we successfully wrote. Used to guarantee
    /// safety in the face of panics or buggy parallel iterators.
    writes: &'c AtomicUsize,

    /// A slice covering the target memory, not yet initialized!
    target: &'c mut [ITEM],
}

pub struct CollectFolder<'c, ITEM: Send + 'c> {
    global_writes: &'c AtomicUsize,
    local_writes: usize,

    /// An iterator over the *uninitialized* target memory.
    target: slice::IterMut<'c, ITEM>,
}


impl<'c, ITEM: Send + 'c> CollectConsumer<'c, ITEM> {
    /// The target memory is considered uninitialized, and will be
    /// overwritten without dropping anything.
    pub fn new(writes: &'c AtomicUsize, target: &'c mut [ITEM]) -> CollectConsumer<'c, ITEM> {
        CollectConsumer {
            writes: writes,
            target: target,
        }
    }
}

impl<'c, ITEM: Send + 'c> Consumer<ITEM> for CollectConsumer<'c, ITEM> {
    type Folder = CollectFolder<'c, ITEM>;
    type Reducer = NoopReducer;
    type Result = ();

    fn cost(&mut self, cost: f64) -> f64 {
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self, NoopReducer) {
        // instances Read in the fields from `self` and then
        // forget `self`, since it has been legitimately consumed
        // (and not dropped during unwinding).
        let CollectConsumer { writes, target } = self;

        // Produce new consumers. Normal slicing ensures that the
        // memory range given to each consumer is disjoint.
        let (left, right) = target.split_at_mut(index);
        (CollectConsumer::new(writes, left), CollectConsumer::new(writes, right), NoopReducer)
    }

    fn into_folder(self) -> CollectFolder<'c, ITEM> {
        CollectFolder {
            global_writes: self.writes,
            local_writes: 0,
            target: self.target.into_iter(),
        }
    }
}

impl<'c, ITEM: Send + 'c> Folder<ITEM> for CollectFolder<'c, ITEM> {
    type Result = ();

    fn consume(mut self, item: ITEM) -> CollectFolder<'c, ITEM> {
        // Compute target pointer and write to it. Safe because the iterator
        // does all the bounds checking; we're only avoiding the target drop.
        let head = self.target.next().expect("too many values pushed to consumer");
        unsafe {
            ptr::write(head, item);
        }

        self.local_writes += 1;
        self
    }

    fn complete(self) {
        assert!(self.target.len() == 0, "too few values pushed to consumer");

        // track total values written
        self.global_writes.fetch_add(self.local_writes, Ordering::Relaxed);
    }
}

/// Pretend to be unindexed for `special_collect_into`,
/// but we should never actually get used that way...
impl<'c, ITEM: Send + 'c> UnindexedConsumer<ITEM> for CollectConsumer<'c, ITEM> {
    fn split_off(&self) -> Self {
        unreachable!("CollectConsumer must be indexed!")
    }
    fn to_reducer(&self) -> Self::Reducer {
        NoopReducer
    }
}
