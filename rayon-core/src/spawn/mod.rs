#[cfg(rayon_unstable)]
use future::{self, Future, RayonFuture};
#[allow(unused_imports)]
use latch::{Latch, SpinLatch};
use job::*;
use registry::Registry;
use std::any::Any;
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
/// propagated to the panic handler registered in the `Configuration`,
/// if any.  See [`Configuration::panic_handler()`][ph] for more
/// details.
///
/// [ph]: struct.Configuration.html#method.panic_handler
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

/// Spawns a future in the static scope, scheduling it to execute on
/// Rayon's threadpool. Returns a new future that can be used to poll
/// for the result. Since this future is executing in the static scope,
/// it cannot hold references to things in the enclosing stack frame;
/// if you would like to hold such references, use [the `scope()`
/// function][scope] to create a scope.
///
/// [scope]: fn.scope.html
///
/// # Panic handling
///
/// If this future should panic, that panic will be propagated when
/// `poll()` is invoked on the return value.
#[cfg(rayon_unstable)]
pub fn spawn_future<F>(future: F) -> RayonFuture<F::Item, F::Error>
    where F: Future + Send + 'static
{
    /// We assert that the current registry cannot yet have terminated.
    unsafe { spawn_future_in(future, Registry::current()) }
}

/// Internal helper function.
///
/// Unsafe because caller must guarantee that `registry` has not yet terminated.
#[cfg(rayon_unstable)]
pub unsafe fn spawn_future_in<F>(future: F, registry: Arc<Registry>) -> RayonFuture<F::Item, F::Error>
    where F: Future + Send + 'static
{
    let scope = StaticFutureScope::new(registry.clone());

    future::new_rayon_future(future, scope)
}

#[cfg(rayon_unstable)]
struct StaticFutureScope {
    registry: Arc<Registry>
}

#[cfg(rayon_unstable)]
impl StaticFutureScope {
    /// Caller asserts that the registry has not yet terminated.
    unsafe fn new(registry: Arc<Registry>) -> Self {
        registry.increment_terminate_count();
        StaticFutureScope { registry: registry }
    }
}

/// We assert that:
///
/// (a) the scope valid remains valid until a completion method
///     is called. In this case, "remains valid" means that the
///     registry is not terminated. This is true because we
///     acquire a "termination count" in `StaticFutureScope::new()`
///     which is not released until `future_panicked()` or
///     `future_completed()` is invoked.
/// (b) the lifetime `'static` will not end until a completion
///     method is called. This is true because `'static` doesn't
///     end until the end of the program.
#[cfg(rayon_unstable)]
unsafe impl future::FutureScope<'static> for StaticFutureScope {
    fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }

    fn future_panicked(self, err: Box<Any + Send>) {
        self.registry.handle_panic(err);
        self.registry.terminate();
    }

    fn future_completed(self) {
        self.registry.terminate();
    }
}

#[cfg(test)]
mod test;
