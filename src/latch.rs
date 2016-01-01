use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Condvar};
use std::thread;

pub trait Latch {
    fn probe(&self) -> bool;
    fn set(&self);
    fn wait(&self);
}

/// A Latch starts as false and eventually becomes true. You can block
/// until it becomes true.
pub struct SpinLatch {
    b: AtomicBool
}

impl SpinLatch {
    #[inline]
    pub fn new() -> SpinLatch {
        SpinLatch {
            b: AtomicBool::new(false)
        }
    }
}

impl Latch for SpinLatch {
    /// Test if latch is set.
    fn probe(&self) -> bool {
        self.b.load(Ordering::Acquire)
    }

    /// Set the latch to true, releasing all threads who are waiting.
    fn set(&self) {
        self.b.store(true, Ordering::Release);
    }

    /// Spin until latch is set. Use with caution.
    fn wait(&self) {
        while !self.probe() {
            thread::yield_now();
        }
    }
}

/// A Latch starts as false and eventually becomes true. You can block
/// until it becomes true.
pub struct LockLatch {
    b: AtomicBool,
    m: Mutex<()>,
    v: Condvar,
}

impl LockLatch {
    #[inline]
    pub fn new() -> LockLatch {
        LockLatch {
            b: AtomicBool::new(false),
            m: Mutex::new(()),
            v: Condvar::new(),
        }
    }
}

impl Latch for LockLatch {
    /// Test if latch is set.
    fn probe(&self) -> bool {
        self.b.load(Ordering::Acquire)
    }

    /// Set the latch to true, releasing all threads who are waiting.
    fn set(&self) {
        self.b.store(true, Ordering::Release);
        self.v.notify_all();
    }

    /// Spin until latch is set. Use with caution.
    fn wait(&self) {
        let mut guard = self.m.lock().unwrap();
        while !self.probe() {
            guard = self.v.wait(guard).unwrap();
        }
    }
}
