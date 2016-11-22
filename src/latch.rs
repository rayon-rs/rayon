use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Condvar};
use std::thread;

pub trait Latch {
    fn set(&self);
}

/// A Latch starts as false and eventually becomes true. You can block
/// until it becomes true.
pub struct SpinLatch {
    b: AtomicBool,
}

impl SpinLatch {
    #[inline]
    pub fn new() -> SpinLatch {
        SpinLatch { b: AtomicBool::new(false) }
    }

    /// Test if latch is set.
    #[inline]
    pub fn probe(&self) -> bool {
        self.b.load(Ordering::Acquire)
    }

    /// Block until latch is set. Use with caution.
    #[inline]
    pub fn spin(&self) {
        while !self.probe() {
            thread::yield_now();
        }
    }
}

impl Latch for SpinLatch {
    /// Set the latch to true, releasing all threads who are waiting.
    #[inline]
    fn set(&self) {
        self.b.store(true, Ordering::Release);
    }
}

/// A Latch starts as false and eventually becomes true. You can block
/// until it becomes true.
pub struct LockLatch {
    m: Mutex<bool>,
    v: Condvar,
}

impl LockLatch {
    #[inline]
    pub fn new() -> LockLatch {
        LockLatch {
            m: Mutex::new(false),
            v: Condvar::new(),
        }
    }

    /// Block until latch is set.
    pub fn wait(&self) {
        let mut guard = self.m.lock().unwrap();
        while !*guard {
            guard = self.v.wait(guard).unwrap();
        }
    }
}

impl Latch for LockLatch {
    /// Set the latch to true, releasing all threads who are waiting.
    #[inline]
    fn set(&self) {
        let mut guard = self.m.lock().unwrap();
        *guard = true;
        self.v.notify_all();
    }
}
