use latch::{Latch, SpinLatch, LockLatch};
use job::{JobMode, HeapJob, StackJob};
use std::any::Any;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicUsize, AtomicPtr, Ordering};
use thread_pool::{self, WorkerThread};
use unwind;

#[cfg(test)]
mod test;

pub struct Scope<'scope> {
    /// thread where `scope()` was executed (note that individual jobs
    /// may be executing on different worker threads, though they
    /// should always be within the same pool of threads)
    owner_thread: *mut WorkerThread,

    /// number of jobs created that have not yet completed or errored
    counter: AtomicUsize,

    /// if some job panicked, the error is stored here; it will be
    /// propagated to the one who created the scope
    panic: AtomicPtr<Box<Any + Send + 'static>>,

    /// latch to set when the counter drops to zero (and hence this scope is complete)
    job_completed_latch: SpinLatch,

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
pub fn scope<'scope, OP>(op: OP)
    where OP: for<'s> FnOnce(&'s Scope<'scope>) + 'scope + Send
{
    unsafe {
        let owner_thread = WorkerThread::current();
        if !owner_thread.is_null() {
            let scope: Scope<'scope> = Scope {
                owner_thread: owner_thread,
                counter: AtomicUsize::new(1),
                panic: AtomicPtr::new(ptr::null_mut()),
                job_completed_latch: SpinLatch::new(),
                marker: PhantomData,
            };
            let spawn_count = (*owner_thread).current_spawn_count();
            scope.execute_job_closure(op);
            (*owner_thread).pop_spawned_jobs(spawn_count);
            scope.steal_till_jobs_complete();
        } else {
            scope_not_in_worker(op)
        }
    }
}

#[cold]
unsafe fn scope_not_in_worker<'scope, OP>(op: OP)
    where OP: for<'s> FnOnce(&'s Scope<'scope>) + 'scope + Send
{
    // never run from a worker thread; just shifts over into worker threads
    debug_assert!(WorkerThread::current().is_null());

    let mut result = None;
    {
        let job = StackJob::new(|| result = Some(scope(op)), LockLatch::new());
        thread_pool::get_registry().inject(&[job.as_job_ref()]);
        job.latch.wait();
    }
    result.unwrap()
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
            let old_value = self.counter.fetch_add(1, Ordering::SeqCst);
            assert!(old_value > 0); // scope can't have completed yet
            let job_ref = Box::new(HeapJob::new(move |mode| self.execute_job(body, mode)))
                .as_job_ref();
            let worker_thread = WorkerThread::current();

            // the `Scope` is not send or sync, and we only give out
            // pointers to it from within a worker thread
            debug_assert!(!WorkerThread::current().is_null());

            let worker_thread = &*worker_thread;
            worker_thread.bump_spawn_count();
            worker_thread.push(job_ref);
        }
    }

    /// Executes `func` as a job, either aborting or executing as
    /// appropriate.
    ///
    /// Unsafe because it must be executed on a worker thread.
    unsafe fn execute_job<FUNC>(&self, func: FUNC, mode: JobMode)
        where FUNC: FnOnce(&Scope<'scope>) + 'scope
    {
        match mode {
            JobMode::Execute => self.execute_job_closure(func),
            JobMode::Abort => self.job_completed_ok(),
        }
    }

    /// Executes `func` as a job in scope. Adjusts the "job completed"
    /// counters and also catches any panic and stores it into
    /// `scope`.
    ///
    /// Unsafe because this must be executed on a worker thread.
    unsafe fn execute_job_closure<FUNC>(&self, func: FUNC)
        where FUNC: FnOnce(&Scope<'scope>) + 'scope
    {
        match unwind::halt_unwinding(move || func(self)) {
            Ok(()) => self.job_completed_ok(),
            Err(err) => self.job_panicked(err),
        }
    }

    unsafe fn job_panicked(&self, err: Box<Any + Send + 'static>) {
        // capture the first error we see, free the rest
        let nil = ptr::null_mut();
        let mut err = Box::new(err); // box up the fat ptr
        if self.panic.compare_and_swap(nil, &mut *err, Ordering::SeqCst).is_null() {
            mem::forget(err); // ownership now transferred into self.panic
        }

        self.job_completed_ok()
    }

    unsafe fn job_completed_ok(&self) {
        let old_value = self.counter.fetch_sub(1, Ordering::Release);
        if old_value == 1 {
            // Important: grab the lock here to avoid a data race with
            // the `block_till_jobs_complete` code. Consider what could
            // otherwise happen:
            //
            // ```
            //    Us          Them
            //              Acquire lock
            //              Read counter: 1
            // Dec counter
            // Notify all
            //              Wait on job_completed_cvar
            // ```
            //
            // By holding the lock, we ensure that the "read counter"
            // and "wait on job_completed_cvar" occur atomically with respect to the
            // notify.
            self.job_completed_latch.set();
        }
    }

    unsafe fn steal_till_jobs_complete(&self) {
        // at this point, we have popped all tasks spawned since the scope
        // began. So either we've executed everything on this thread, or one of
        // those was stolen. If one of them was stolen, then everything below us on
        // the deque must have been stolen too, so we should just go ahead and steal.
        debug_assert!(self.job_completed_latch.probe() || (*self.owner_thread).pop().is_none());

        // wait for job counter to reach 0:
        (*self.owner_thread).steal_until(&self.job_completed_latch);

        // propagate panic, if any occurred; at this point, all
        // outstanding jobs have completed, so we can use a relaxed
        // ordering:
        let panic = self.panic.swap(ptr::null_mut(), Ordering::Relaxed);
        if !panic.is_null() {
            let value: Box<Box<Any + Send + 'static>> = mem::transmute(panic);
            unwind::resume_unwinding(*value);
        }
    }
}
