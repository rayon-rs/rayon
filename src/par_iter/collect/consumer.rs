use super::super::len::*;
use super::super::internal::*;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct CollectConsumer<ITEM: Send> {
    target: *mut ITEM,

    /// Number of items pushed to the consumer so far. For simplicity,
    /// can only split when this is zero.
    ///
    /// Invariant: `self.offset <= self.len`
    offset: usize,

    /// Invariant: we can safely write from `target` .. `target+len`
    len: usize,
}

/// Safe to send because all `CollectConsumer` instances have disjoint
/// `target` regions (as limited by `len`).
unsafe impl<ITEM: Send> Send for CollectConsumer<ITEM> { }

impl<ITEM: Send> CollectConsumer<ITEM> {
    /// Unsafe because caller must assert that `target..target+len` is
    /// an unaliased, writable region.
    pub unsafe fn new(target: *mut ITEM, len: usize) -> CollectConsumer<ITEM> {
        CollectConsumer { target: target, offset: 0, len: len }
    }
}

impl<'c, ITEM: Send + 'c> Consumer<'c> for CollectConsumer<ITEM> {
    type Item = ITEM;
    type Shared = AtomicUsize;
    type SeqState = ();
    type Result = ();

    fn cost(&mut self, _: &Self::Shared, cost: f64) -> f64 {
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _: &Self::Shared, index: usize) -> (Self, Self) {
        // instances Read in the fields from `self` and then
        // forget `self`, since it has been legitimately consumed
        // (and not dropped during unwinding).
        let CollectConsumer { target, offset, len } = self;
        mem::forget(self);

        // Check that user is using the protocol correctly.
        assert!(offset == 0, "cannot split after consuming an item");
        assert!(index < len, "out of bounds index in collect");

        // Produce new consumers. Here we are asserting that the
        // memory range given to each consumer is disjoint.
        unsafe {
            (CollectConsumer::new(target, index),
             CollectConsumer::new(target.offset(index as isize), len - index))
        }
    }

    fn start(&mut self, _: &AtomicUsize) {
    }

    fn consume(&mut self, _: &AtomicUsize, _: (), item: ITEM) {
        // Assert that struct invariants hold, and hence computing
        // `target` and writing to it is valid.
        assert!(self.offset < self.len, "too many values pushed to consumer");

        // Compute target pointer and write to it. Safe because of
        // struct invariants and because we asserted that `offset < len`.
        unsafe {
            let target = self.target.offset(self.offset as isize);
            ptr::write(target, item);
        }

        // Maintains struct invariants because we know `offset < len`.
        self.offset += 1;
    }

    fn complete(self, writes: &AtomicUsize, _: ()) {
        let CollectConsumer { target: _, offset, len } = self;
        mem::forget(self);

        assert!(offset == len, "too few values pushed to consumer");

        // track total values written
        writes.fetch_add(len, Ordering::SeqCst);
    }

    fn reduce(_: &AtomicUsize, _: (), _: ()) {
    }
}

impl<ITEM: Send> Drop for CollectConsumer<ITEM> {
    fn drop(&mut self) {
        // The only case where we would get dropped is either if an
        // unwind is occurring or if someone forgot to call `complete`.
        panic!("complete() never called in collect"); // TODO only do this if not unwinding
    }
}
