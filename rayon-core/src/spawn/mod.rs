use job::*;
use registry::Registry;
use std::mem;
use std::sync::Arc;
use unwind;

/// Fires off a task into the Rayon threadpool in the "static" or
/// "global" scope.  Just like a standard thread, this task is not
/// tied to the current stack frame, and hence it cannot hold any
/// references other than those with `'static` lifetime. If you want
/// to spawn a task that references stack data, use [the `scope()`
/// function][scope] to create a scope.
///
/// [scope]: fn.scope.html
///
/// Since tasks spawned with this function cannot hold references into
/// the enclosing stack frame, you almost certainly want to use a
/// `move` closure as their argument (otherwise, the closure will
/// typically hold references to any variables from the enclosing
/// function that you happen to use).
///
/// This API assumes that the closure is executed purely for its
/// side-effects (i.e., it might send messages, modify data protected
/// by a mutex, or some such thing). If you want to compute a result,
/// consider `spawn_future()`.
///
/// # Panic handling
///
/// If this closure should panic, the resulting panic will be
/// propagated to the panic handler registered in the `ThreadPoolBuilder`,
/// if any.  See [`ThreadPoolBuilder::panic_handler()`][ph] for more
/// details.
///
/// [ph]: struct.ThreadPoolBuilder.html#method.panic_handler
///
/// # Examples
///
/// This code creates a Rayon task that increments a global counter.
///
/// ```rust
/// # use rayon_core as rayon;
/// use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
///
/// static GLOBAL_COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;
///
/// rayon::spawn(move || {
///     GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
/// });
/// ```
pub fn spawn<F>(func: F)
    where F: FnOnce() + Send + 'static
{
    // We assert that current registry has not terminated.
    unsafe { spawn_in(func, &Registry::current()) }
}

/// Spawn an asynchronous job in `registry.`
///
/// Unsafe because `registry` must not yet have terminated.
///
/// Not a public API, but used elsewhere in Rayon.
pub unsafe fn spawn_in<F>(func: F, registry: &Arc<Registry>)
    where F: FnOnce() + Send + 'static
{
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
    registry.inject_or_push(job_ref);
    mem::forget(abort_guard);
}

#[cfg(test)]
mod test;
