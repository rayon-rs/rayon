#[cfg(rayon_unstable)]
use future::{self, Future, RayonFuture};
use latch::{Latch, CountLatch};
use log::Event::*;
use job::HeapJob;
use std::any::Any;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use registry::{in_worker, Registry, WorkerThread};
use unwind;

#[cfg(test)]
mod test;

pub struct Scope<'scope> {
    /// thread where `scope()` was executed (note that individual jobs
    /// may be executing on different worker threads, though they
    /// should always be within the same pool of threads)
    owner_thread: *const WorkerThread,

    /// if some job panicked, the error is stored here; it will be
    /// propagated to the one who created the scope
    panic: AtomicPtr<Box<Any + Send + 'static>>,

    /// latch to set when the counter drops to zero (and hence this scope is complete)
    job_completed_latch: CountLatch,

    /// you can think of a scope as containing a list of closures to
    /// execute, all of which outlive `'scope`
    marker: PhantomData<Box<FnOnce(&Scope<'scope>) + 'scope>>,
}

/// Create a "fork-join" scope `s` and invokes the closure with a
/// reference to `s`. This closure can then spawn asynchronous tasks
/// into `s`. Those tasks may run asynchronously with respect to the
/// closure; they may themselves spawn additional tasks into `s`. When
/// the closure returns, it will block until all tasks that have been
/// spawned into `s` complete.
///
/// `scope()` is a more flexible building block compared to `join()`,
/// since a loop can be used to spawn any number of tasks without
/// recursing. However, that flexibility comes at a performance price:
/// tasks spawned using `scope()` must be allocated onto the heap,
/// whereas `join()` can make exclusive use of the stack. **Prefer
/// `join()` (or, even better, parallel iterators) where possible.**
///
/// ### Example
///
/// The Rayon `join()` function launches two closures and waits for them
/// to stop. One could implement `join()` using a scope like so, although
/// it would be less efficient than the real implementation:
///
/// ```rust
/// # use rayon_core as rayon;
/// pub fn join<A,B,RA,RB>(oper_a: A, oper_b: B) -> (RA, RB)
///     where A: FnOnce() -> RA + Send,
///           B: FnOnce() -> RB + Send,
///           RA: Send,
///           RB: Send,
/// {
///     let mut result_a: Option<RA> = None;
///     let mut result_b: Option<RB> = None;
///     rayon::scope(|s| {
///         s.spawn(|_| result_a = Some(oper_a()));
///         s.spawn(|_| result_b = Some(oper_b()));
///     });
///     (result_a.unwrap(), result_b.unwrap())
/// }
/// ```
///
/// ### A note on threading
///
/// The closure given to `scope()` executes in the Rayon thread-pool,
/// as do those given to `spawn()`. This means that you can't access
/// thread-local variables (well, you can, but they may have
/// unexpected values).
///
/// ### Task execution
///
/// Task execution potentially starts as soon as `spawn()` is called.
/// The task will end sometime before `scope()` returns. Note that the
/// *closure* given to scope may return much earlier. In general
/// the lifetime of a scope created like `scope(body) goes something like this:
///
/// - Scope begins when `scope(body)` is called
/// - Scope body `body()` is invoked
///     - Scope tasks may be spawned
/// - Scope body returns
/// - Scope tasks execute, possibly spawning more tasks
/// - Once all tasks are done, scope ends and `scope()` returns
///
/// To see how and when tasks are joined, consider this example:
///
/// ```rust
/// # use rayon_core as rayon;
/// // point start
/// rayon::scope(|s| {
///     s.spawn(|s| { // task s.1
///         s.spawn(|s| { // task s.1.1
///             rayon::scope(|t| {
///                 t.spawn(|_| ()); // task t.1
///                 t.spawn(|_| ()); // task t.2
///             });
///         });
///     });
///     s.spawn(|s| { // task 2
///     });
///     // point mid
/// });
/// // point end
/// ```
///
/// The various tasks that are run will execute roughly like so:
///
/// ```notrust
/// | (start)
/// |
/// | (scope `s` created)
/// +--------------------+ (task s.1)
/// +-------+ (task s.2) |
/// |       |            +---+ (task s.1.1)
/// |       |            |   |
/// |       |            |   | (scope `t` created)
/// |       |            |   +----------------+ (task t.1)
/// |       |            |   +---+ (task t.2) |
/// | (mid) |            |   |   |            |
/// :       |            |   + <-+------------+ (scope `t` ends)
/// :       |            |   |
/// |<------+------------+---+ (scope `s` ends)
/// |
/// | (end)
/// ```
///
/// The point here is that everything spawned into scope `s` will
/// terminate (at latest) at the same point -- right before the
/// original call to `rayon::scope` returns. This includes new
/// subtasks created by other subtasks (e.g., task `s.1.1`). If a new
/// scope is created (such as `t`), the things spawned into that scope
/// will be joined before that scope returns, which in turn occurs
/// before the creating task (task `s.1.1` in this case) finishes.
///
/// ### Accessing stack data
///
/// In general, spawned tasks may access stack data in place that
/// outlives the scope itself. Other data must be fully owned by the
/// spawned task.
///
/// ```rust
/// # use rayon_core as rayon;
/// let ok: Vec<i32> = vec![1, 2, 3];
/// rayon::scope(|s| {
///     let bad: Vec<i32> = vec![4, 5, 6];
///     s.spawn(|_| {
///         // We can access `ok` because outlives the scope `s`.
///         println!("ok: {:?}", ok);
///
///         // If we just try to use `bad` here, the closure will borrow `bad`
///         // (because we are just printing it out, and that only requires a
///         // borrow), which will result in a compilation error. Read on
///         // for options.
///         // println!("bad: {:?}", bad);
///    });
/// });
/// ```
///
/// As the comments example above suggest, to reference `bad` we must
/// take ownership of it. One way to do this is to detach the closure
/// from the surrounding stack frame, using the `move` keyword. This
/// will cause it to take ownership of *all* the variables it touches,
/// in this case including both `ok` *and* `bad`:
///
/// ```rust
/// # use rayon_core as rayon;
/// let ok: Vec<i32> = vec![1, 2, 3];
/// rayon::scope(|s| {
///     let bad: Vec<i32> = vec![4, 5, 6];
///     s.spawn(move |_| {
///         println!("ok: {:?}", ok);
///         println!("bad: {:?}", bad);
///     });
///
///     // That closure is fine, but now we can't use `ok` anywhere else,
///     // since it is owend by the previous task:
///     // s.spawn(|_| println!("ok: {:?}", ok));
/// });
/// ```
///
/// While this works, it could be a problem if we want to use `ok` elsewhere.
/// There are two choices. We can keep the closure as a `move` closure, but
/// instead of referencing the variable `ok`, we create a shadowed variable that
/// is a borrow of `ok` and capture *that*:
///
/// ```rust
/// # use rayon_core as rayon;
/// let ok: Vec<i32> = vec![1, 2, 3];
/// rayon::scope(|s| {
///     let bad: Vec<i32> = vec![4, 5, 6];
///     let ok: &Vec<i32> = &ok; // shadow the original `ok`
///     s.spawn(move |_| {
///         println!("ok: {:?}", ok); // captures the shadowed version
///         println!("bad: {:?}", bad);
///     });
///
///     // Now we too can use the shadowed `ok`, since `&Vec<i32>` references
///     // can be shared freely. Note that we need a `move` closure here though,
///     // because otherwise we'd be trying to borrow the shadowed `ok`,
///     // and that doesn't outlive `scope`.
///     s.spawn(move |_| println!("ok: {:?}", ok));
/// });
/// ```
///
/// Another option is not to use the `move` keyword but instead to take ownership
/// of individual variables:
///
/// ```rust
/// # use rayon_core as rayon;
/// let ok: Vec<i32> = vec![1, 2, 3];
/// rayon::scope(|s| {
///     let bad: Vec<i32> = vec![4, 5, 6];
///     s.spawn(|_| {
///         // Transfer ownership of `bad` into a local variable (also named `bad`).
///         // This will force the closure to take ownership of `bad` from the environment.
///         let bad = bad;
///         println!("ok: {:?}", ok); // `ok` is only borrowed.
///         println!("bad: {:?}", bad); // refers to our local variable, above.
///     });
///
///     s.spawn(|_| println!("ok: {:?}", ok)); // we too can borrow `ok`
/// });
/// ```
///
/// ### Panics
///
/// If a panic occurs, either in the closure given to `scope()` or in
/// any of the spawned jobs, that panic will be propagated and the
/// call to `scope()` will panic. If multiple panics occurs, it is
/// non-deterministic which of their panic values will propagate.
/// Regardless, once a task is spawned using `scope.spawn()`, it will
/// execute, even if the spawning task should later panic. `scope()`
/// returns once all spawned jobs have completed, and any panics are
/// propagated at that point.
pub fn scope<'scope, OP, R>(op: OP) -> R
    where OP: for<'s> FnOnce(&'s Scope<'scope>) -> R + 'scope + Send, R: Send,
{
    in_worker(|owner_thread| {
        unsafe {
            let scope: Scope<'scope> = Scope {
                owner_thread: owner_thread as *const WorkerThread as *mut WorkerThread,
                panic: AtomicPtr::new(ptr::null_mut()),
                job_completed_latch: CountLatch::new(),
                marker: PhantomData,
            };
            let result = scope.execute_job_closure(op);
            scope.steal_till_jobs_complete();
            result.unwrap() // only None if `op` panicked, and that would have been propagated
        }
    })
}

impl<'scope> Scope<'scope> {
    /// Spawns a job into the fork-join scope `self`. This job will
    /// execute sometime before the fork-join scope completes.  The
    /// job is specified as a closure, and this closure receives its
    /// own reference to `self` as argument. This can be used to
    /// inject new jobs into `self`.
    pub fn spawn<BODY>(&self, body: BODY)
        where BODY: FnOnce(&Scope<'scope>) + 'scope
    {
        unsafe {
            self.job_completed_latch.increment();
            let job_ref = Box::new(HeapJob::new(move || self.execute_job(body)))
                .as_job_ref();
            let worker_thread = WorkerThread::current();

            // the `Scope` is not send or sync, and we only give out
            // pointers to it from within a worker thread
            debug_assert!(!WorkerThread::current().is_null());

            let worker_thread = &*worker_thread;
            worker_thread.push(job_ref);
        }
    }

    #[cfg(rayon_unstable)]
    pub fn spawn_future<F>(&self, future: F) -> RayonFuture<F::Item, F::Error>
        where F: Future + Send + 'scope
    {
        // We assert that the scope is allocated in a stable location
        // (an enclosing stack frame, to be exact) which will remain
        // valid until the scope ends.
        let future_scope = unsafe { ScopeFutureScope::new(self) };

        return future::new_rayon_future(future, future_scope);

        struct ScopeFutureScope<'scope> {
            scope: *const Scope<'scope>
        }

        impl<'scope> ScopeFutureScope<'scope> {
            /// Caller guarantees that `*scope` will remain valid
            /// until the scope completes. Since we acquire a ref,
            /// that means it will remain valid until we release it.
            unsafe fn new(scope: &Scope<'scope>) -> Self {
                scope.job_completed_latch.increment();
                ScopeFutureScope { scope: scope }
            }
        }

        /// We assert that the `Self` type remains valid until a
        /// method is called, and that `'scope` will not end until
        /// that point.
        unsafe impl<'scope> future::FutureScope<'scope> for ScopeFutureScope<'scope> {
            fn registry(&self) -> Arc<Registry> {
                unsafe {
                    (*(*self.scope).owner_thread).registry().clone()
                }
            }

            fn future_completed(self) {
                unsafe {
                    (*self.scope).job_completed_ok();
                }
            }

            fn future_panicked(self, err: Box<Any + Send>) {
                unsafe {
                    (*self.scope).job_panicked(err);
                }
            }
        }
    }

    /// Executes `func` as a job, either aborting or executing as
    /// appropriate.
    ///
    /// Unsafe because it must be executed on a worker thread.
    unsafe fn execute_job<FUNC>(&self, func: FUNC)
        where FUNC: FnOnce(&Scope<'scope>) + 'scope
    {
        let _: Option<()> = self.execute_job_closure(func);
    }

    /// Executes `func` as a job in scope. Adjusts the "job completed"
    /// counters and also catches any panic and stores it into
    /// `scope`.
    ///
    /// Unsafe because this must be executed on a worker thread.
    unsafe fn execute_job_closure<FUNC, R>(&self, func: FUNC) -> Option<R>
        where FUNC: FnOnce(&Scope<'scope>) -> R + 'scope
    {
        match unwind::halt_unwinding(move || func(self)) {
            Ok(r) => { self.job_completed_ok(); Some(r) }
            Err(err) => { self.job_panicked(err); None }
        }
    }

    unsafe fn job_panicked(&self, err: Box<Any + Send + 'static>) {
        // capture the first error we see, free the rest
        let nil = ptr::null_mut();
        let mut err = Box::new(err); // box up the fat ptr
        if self.panic.compare_exchange(nil, &mut *err, Ordering::Release, Ordering::Relaxed).is_ok() {
            log!(JobPanickedErrorStored { owner_thread: (*self.owner_thread).index() });
            mem::forget(err); // ownership now transferred into self.panic
        } else {
            log!(JobPanickedErrorNotStored { owner_thread: (*self.owner_thread).index() });
        }


        self.job_completed_latch.set();
    }

    unsafe fn job_completed_ok(&self) {
        log!(JobCompletedOk { owner_thread: (*self.owner_thread).index() });
        self.job_completed_latch.set();
    }

    unsafe fn steal_till_jobs_complete(&self) {
        // wait for job counter to reach 0:
        (*self.owner_thread).wait_until(&self.job_completed_latch);

        // propagate panic, if any occurred; at this point, all
        // outstanding jobs have completed, so we can use a relaxed
        // ordering:
        let panic = self.panic.swap(ptr::null_mut(), Ordering::Relaxed);
        if !panic.is_null() {
            log!(ScopeCompletePanicked { owner_thread: (*self.owner_thread).index() });
            let value: Box<Box<Any + Send + 'static>> = mem::transmute(panic);
            unwind::resume_unwinding(*value);
        } else {
            log!(ScopeCompleteNoPanic { owner_thread: (*self.owner_thread).index() });
        }
    }
}
