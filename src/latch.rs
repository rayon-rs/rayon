use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// A Latch starts as false and eventually becomes true. You can block
/// until it becomes true.
pub struct Latch {
    b: AtomicBool
}

impl Latch {
    #[inline]
    pub fn new() -> Latch {
        Latch {
            b: AtomicBool::new(false)
        }
    }

    /// Set the latch to true, releasing all threads who are waiting.
    pub fn set(&self) {
        self.b.store(true, Ordering::SeqCst);
    }

    /// Spin until latch is set. Use with caution.
    pub fn wait(&self) {
        while !self.probe() {
            thread::yield_now();
        }
    }

    /// Test if latch is set.
    pub fn probe(&self) -> bool {
        self.b.load(Ordering::SeqCst)
    }
}
