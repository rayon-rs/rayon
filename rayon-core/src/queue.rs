use crossbeam_queue::{ArrayQueue, PopError, SegQueue};

/// A data exchange structure that tries to be a queue and almost succeeds.
pub(crate) struct AlmostQueue<T> {
    unbounded: SegQueue<T>,
    bounded: ArrayQueue<T>,
}

impl<T> AlmostQueue<T> {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            unbounded: SegQueue::new(),
            bounded: ArrayQueue::new(cap),
        }
    }
    
    pub(crate) fn push(&self, t: T) {
        // If unbounded has any elements, it always takes over receiving elements.
        if !self.unbounded.is_empty() {
            // Once we entered here, bounded might have become empty again, and a newer element
            // already be on the way to the bounded queue. That newer element would move to the
            // bounded queue, and be handled before our element we just entered.
            self.unbounded.push(t);
        } else {
            // If we enter here, unbounded might have become inhabited again with a newer element.
            // In that case we would append our older element to the bounded queue. However, that
            // would be fine, since we always process the bounded queue first.
            if let Err(e) = self.bounded.push(t) {
                // In this pathological case, we wanted to have inserted our older element
                // into bounded, but couldn't because another element was stealing our slot.
                // In that case we flow over to unbounded again, and our order might be out of
                // place until all the elements in between have been processed.
                self.unbounded.push(e.0);
            }
        }
    }
    
    pub(crate) fn pop(&self) -> Result<T, PopError> {
        // We always pop the bounded first. It is filled first when the queue is empty,
        // and might spuriously be filled if popping happens during pushing.
        let x = self.bounded.pop();
        
        if x.is_ok() {
            x
        } else {
            self.unbounded.pop()
        }
    }
}
