use latch::LatchProbe;
use registry;

/// Represents the active worker thread.
pub struct WorkerThread<'w> {
    thread: &'w registry::WorkerThread
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
    pub unsafe fn wait_until_true<F>(&self, f: F) where F: Fn() -> bool {
        struct DummyLatch<'a, F: 'a> { f: &'a F }

        impl<'a, F: Fn() -> bool> LatchProbe for DummyLatch<'a, F> {
            fn probe(&self) -> bool {
                (self.f)()
            }
        }

        self.thread.wait_until(&DummyLatch { f: &f });
    }
}

/// If the current thread is a Rayon worker thread, then the callback
/// is invoked with a reference to the worker-thread the result of
/// that callback is returned with `Some`. Otherwise, if we are not on
/// a Rayon worker thread, `None` is immediately returned.
pub fn if_in_worker_thread<F,R>(if_true: F) -> Option<R>
    where F: FnOnce(&WorkerThread) -> R,
{
    let worker_thread = registry::WorkerThread::current();
    if worker_thread.is_null() {
        None
    } else {
        unsafe {
            let wt = WorkerThread { thread: &*worker_thread };
            Some(if_true(&wt))
        }
    }
}

