#[allow(unused_imports)]
use latch::{Latch, SpinLatch};
use job::*;
use registry::{Registry, WorkerThread};
use std::mem;
use std::sync::Arc;
use unwind;

/// Fires off a task into the Rayon threadpool that will run
/// asynchronously. As this task runs asynchronously, it cannot hold
/// any references to the enclosing stack frame. Like a regular Rust
/// thread, it should always be created with a `move` closure and
/// should not hold any references (except for `&'static` references).
///
/// This API assumes that the closure is executed purely for its
/// side-effects (i.e., it might send messages, modify data protected
/// by a mutex, or some such thing). If you want to compute a result,
/// consider `spawn_future_async()`.
///
/// # Panic handling
///
/// If this closure should panic, the resulting panic will be
/// propagated to the panic handler registered in the `Configuration`,
/// if any.  See `Configuration::set_panic_handler()` for more
/// details.
///
/// # Examples
///
/// This code creates a Rayon task that increments a global counter.
///
/// ```rust
/// use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
///
/// static GLOBAL_COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;
///
/// rayon::spawn_async(move || {
///     GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
/// });
/// ```
pub fn spawn_async<F>(func: F)
    where F: FnOnce() + Send + 'static
{
    spawn_async_in(func, &Registry::current());
}

// Not a public API, but used elsewhere in Rayon.
pub fn spawn_async_in<F>(func: F, registry: &Arc<Registry>)
    where F: FnOnce() + Send + 'static
{
    unsafe {
        // Ensure that registry cannot terminate until this job has
        // executed. This ref is decremented at the (*) below.
        registry.increment_terminate_count();

        let async_job = Box::new(HeapJob::new({
            let registry = registry.clone();
            move || {
                match unwind::halt_unwinding(func) {
                    Ok(()) => {
                    }
                    Err(err) => {
                        registry.handle_panic(err);
                    }
                }
                registry.terminate(); // (*) permit registry to terminate now
            }
        }));

        // We assert that this does not hold any references (we know
        // this because of the `'static` bound in the inferface);
        // moreover, we assert that the code below is not supposed to
        // be able to panic, and hence the data won't leak but will be
        // enqueued into some deque for later execution.
        let abort_guard = unwind::AbortIfPanic; // just in case we are wrong, and code CAN panic
        let job_ref = HeapJob::as_job_ref(async_job);
        let worker_thread = WorkerThread::current();
        if !worker_thread.is_null() && (*worker_thread).registry().id() == registry.id() {
            (*worker_thread).push(job_ref);
        } else {
            registry.inject(&[job_ref]);
        }
        mem::forget(abort_guard);
    }
}


#[cfg(test)]
mod test;
