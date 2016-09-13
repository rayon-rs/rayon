use super::super::len::*;
use super::super::internal::*;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct CollectConsumer<'c, ITEM: Send> {
    /// Tracks how many items we successfully wrote. Used to guarantee
    /// safety in the face of panics or buggy parallel iterators.
    writes: &'c AtomicUsize,

    target: *mut ITEM,

    /// Invariant: we can safely write from `target` .. `target+len`
    len: usize,
}

pub struct CollectFolder<'c, ITEM: Send> {
    consumer: CollectConsumer<'c, ITEM>,

    /// Number of items pushed to the consumer so far. For simplicity,
    /// can only split when this is zero.
    ///
    /// Invariant: `self.offset <= self.len`
    offset: usize,
}


/// Safe to send because all `CollectConsumer` instances have disjoint
/// `target` regions (as limited by `len`).
unsafe impl<'c, ITEM: Send> Send for CollectConsumer<'c, ITEM> { }

impl<'c, ITEM: Send> CollectConsumer<'c, ITEM> {
    /// Unsafe because caller must assert that `target..target+len` is
    /// an unaliased, writable region.
    pub unsafe fn new(writes: &'c AtomicUsize,
                      target: *mut ITEM,
                      len: usize) -> CollectConsumer<ITEM> {
        CollectConsumer { writes: writes, target: target, len: len }
    }
}

impl<'c, ITEM: Send> Consumer<ITEM> for CollectConsumer<'c, ITEM> {
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
        let CollectConsumer { writes, target, len } = self;

        // Check that user is using the protocol correctly.
        assert!(index <= len, "out of bounds index in collect");

        // Produce new consumers. Here we are asserting that the
        // memory range given to each consumer is disjoint.
        unsafe {
            (CollectConsumer::new(writes, target, index),
             CollectConsumer::new(writes, target.offset(index as isize), len - index),
             NoopReducer)
        }
    }

    fn into_folder(self) -> CollectFolder<'c, ITEM> {
        CollectFolder { consumer: self, offset: 0 }
    }
}

impl<'c, ITEM: Send> Folder<ITEM> for CollectFolder<'c, ITEM> {
    type Result = ();

    fn consume(mut self, item: ITEM) -> CollectFolder<'c, ITEM> {
        // Assert that struct invariants hold, and hence computing
        // `target` and writing to it is valid.
        assert!(self.offset < self.consumer.len, "too many values pushed to consumer");

        // Compute target pointer and write to it. Safe because of
        // struct invariants and because we asserted that `offset < len`.
        unsafe {
            let target = self.consumer.target.offset(self.offset as isize);
            ptr::write(target, item);
        }

        // Maintains struct invariants because we know `offset < len`.
        self.offset += 1;

        self
    }

    fn complete(self) {
        assert!(self.offset == self.consumer.len, "too few values pushed to consumer");

        // track total values written
        self.consumer.writes.fetch_add(self.consumer.len, Ordering::SeqCst);
    }
}
