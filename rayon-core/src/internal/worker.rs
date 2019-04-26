//! Internal, unsafe APIs for manipulating or querying the current
//! worker thread. Intended for building abstractions atop the
//! rayon-core thread pool, rather than direct use by end users.

use latch::LatchProbe;
use registry;
use std::fmt;

/// Represents the active worker thread.
pub struct WorkerThread<'w> {
    thread: &'w registry::WorkerThread,
}

impl<'w> WorkerThread<'w> {
    /// Causes the worker thread to wait until `f()` returns true.
    /// While the thread is waiting, it will attempt to steal work
    /// from other threads, and may go to sleep if there is no work to
    /// steal.
    ///
    /// **Dead-lock warning: This is a low-level interface and cannot
    /// be used to wait on arbitrary conditions.** In particular, if
    /// the Rayon thread goes to sleep, it will only be awoken when
    /// new rayon events occur (e.g., `spawn()` or `join()` is
    /// invoked, or one of the methods on a `ScopeHandle`). Therefore,
    /// you must ensure that, once the condition `f()` becomes true,
    /// some "rayon event" will also occur to ensure that waiting
    /// threads are awoken.
    pub unsafe fn wait_until_true<F>(&self, f: F)
    where
        F: Fn() -> bool,
    {
        struct DummyLatch<'a, F: 'a> {
            f: &'a F,
        }

        impl<'a, F: Fn() -> bool> LatchProbe for DummyLatch<'a, F> {
            fn probe(&self) -> bool {
                (self.f)()
            }
        }

        self.thread.wait_until(&DummyLatch { f: &f });
    }
}

impl<'w> fmt::Debug for WorkerThread<'w> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("WorkerThread")
            .field("pool", &self.thread.registry().id())
            .field("index", &self.thread.index())
            .finish()
    }
}

/// If the current thread is a Rayon worker thread, then the callback
/// is invoked with a reference to the worker-thread the result of
/// that callback is returned with `Some`. Otherwise, if we are not on
/// a Rayon worker thread, `None` is immediately returned.
pub fn if_in_worker_thread<F, R>(if_true: F) -> Option<R>
where
    F: FnOnce(&WorkerThread) -> R,
{
    unsafe {
        let thread = registry::WorkerThread::current().as_ref()?;
        Some(if_true(&WorkerThread { thread }))
    }
}
