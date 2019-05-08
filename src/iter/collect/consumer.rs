use super::super::noop::*;
use super::super::plumbing::*;
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct CollectConsumer<'c, T: Send + 'c> {
    /// Tracks how many items we successfully wrote. Used to guarantee
    /// safety in the face of panics or buggy parallel iterators.
    ///
    /// In theory we could just produce this as a `CollectConsumer::Result`,
    /// folding local counts and reducing by addition, but that requires a
    /// certain amount of trust that the producer driving this will behave
    /// itself. Since this count is important to the safety of marking the
    /// memory initialized (`Vec::set_len`), we choose to keep it internal.
    writes: &'c AtomicUsize,

    /// A slice covering the target memory, not yet initialized!
    target: &'c mut [T],
}

pub(super) struct CollectFolder<'c, T: Send + 'c> {
    global_writes: &'c AtomicUsize,
    local_writes: usize,

    /// An iterator over the *uninitialized* target memory.
    target: slice::IterMut<'c, T>,
}

impl<'c, T: Send + 'c> CollectConsumer<'c, T> {
    /// The target memory is considered uninitialized, and will be
    /// overwritten without dropping anything.
    pub(super) fn new(writes: &'c AtomicUsize, target: &'c mut [T]) -> Self {
        CollectConsumer { writes, target }
    }
}

impl<'c, T: Send + 'c> Consumer<T> for CollectConsumer<'c, T> {
    type Folder = CollectFolder<'c, T>;
    type Reducer = NoopReducer;
    type Result = ();

    fn split_at(self, index: usize) -> (Self, Self, NoopReducer) {
        // instances Read in the fields from `self` and then
        // forget `self`, since it has been legitimately consumed
        // (and not dropped during unwinding).
        let CollectConsumer { writes, target } = self;

        // Produce new consumers. Normal slicing ensures that the
        // memory range given to each consumer is disjoint.
        let (left, right) = target.split_at_mut(index);
        (
            CollectConsumer::new(writes, left),
            CollectConsumer::new(writes, right),
            NoopReducer,
        )
    }

    fn into_folder(self) -> CollectFolder<'c, T> {
        CollectFolder {
            global_writes: self.writes,
            local_writes: 0,
            target: self.target.iter_mut(),
        }
    }

    fn full(&self) -> bool {
        false
    }
}

impl<'c, T: Send + 'c> Folder<T> for CollectFolder<'c, T> {
    type Result = ();

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

        self.local_writes += 1;
        self
    }

    fn complete(self) {
        // NB: We don't explicitly check that the local writes were complete,
        // but `Collect::complete()` will assert the global write count.

        // track total values written
        self.global_writes
            .fetch_add(self.local_writes, Ordering::Relaxed);
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
        NoopReducer
    }
}
