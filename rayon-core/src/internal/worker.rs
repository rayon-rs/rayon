use std::fmt;
use latch::LatchProbe;
use registry;

/// Represents the active worker thread.
pub struct WorkerThread<'w> {
    thread: &'w registry::WorkerThread
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

