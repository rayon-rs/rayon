use crossbeam_queue::{ArrayQueue, PopError, SegQueue};

/// A data exchange structure that tries to be a queue and almost succeeds.
///
/// This structure is useful if you want a possibly unbounded queue, and are willing to trade
/// strict ordering for predictable allocations up to a certain point. In particular, if you
/// load fewer than `cap` elements, this structure will not allocate.
///
/// Also, up to a load of `cap` the structure will behave like a regular, but pre-allocated queue.
/// Beyond that, the ordering of elements might be random until the load falls below `cap` again.
pub(super) struct AlmostQueue<T> {
    unbounded: SegQueue<T>,
    bounded: ArrayQueue<T>,
}

impl<T> AlmostQueue<T> {
    pub(super) fn new(cap: usize) -> Self {
        Self {
            unbounded: SegQueue::new(),
            bounded: ArrayQueue::new(cap),
        }
    }

    pub(super) fn push(&self, t: T) {
        if let Err(x) = self.bounded.push(t) {
            // At this point we know we failed to insert into the bounded queue. In the meantime
            // (between the Err and the next line) the bounded one might have become empty again,
            // but we insert into the unbounded one anyway.
            self.unbounded.push(x.0)
        }
    }

    pub(super) fn pop(&self) -> Result<T, PopError> {
        // We have to query the bounded one first, since it might contain overflow elements even if
        // the bounded one is empty.
        let unbounded = self.unbounded.pop();

        if unbounded.is_ok() {
            unbounded
        } else {
            // We only return unbounded elements if we are sure there are no unbounded elements left.
            self.bounded.pop()
        }
    }
}
