use crossbeam_deque::{Steal, Stealer, Worker};
use crossbeam_queue::SegQueue;
#[cfg(rayon_unstable)]
use internal::task::Task;
#[cfg(rayon_unstable)]
use job::Job;
use job::{JobFifo, JobRef, StackJob};
use latch::{CountLatch, Latch, LatchProbe, LockLatch, SpinLatch, TickleLatch};
use log::Event::*;
use sleep::Sleep;
use std::any::Any;
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;
use std::io;
use std::mem;
use std::ptr;
#[allow(deprecated)]
use std::sync::atomic::ATOMIC_USIZE_INIT;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Once};
use std::thread;
use std::usize;
use unwind;
use util::leak;
use {ErrorKind, ExitHandler, PanicHandler, StartHandler, ThreadPoolBuildError, ThreadPoolBuilder};

/// Thread builder used for customization via
/// [`ThreadPoolBuilder::spawn_handler`](struct.ThreadPoolBuilder.html#method.spawn_handler).
pub struct ThreadBuilder {
    name: Option<String>,
    stack_size: Option<usize>,
    worker: Worker<JobRef>,
    registry: Arc<Registry>,
    index: usize,
}

impl ThreadBuilder {
    /// Get the index of this thread in the pool, within `0..num_threads`.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Get the string that was specified by `ThreadPoolBuilder::name()`.
    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(String::as_str)
    }

    /// Get the value that was specified by `ThreadPoolBuilder::stack_size()`.
    pub fn stack_size(&self) -> Option<usize> {
        self.stack_size
    }

    /// Execute the main loop for this thread. This will not return until the
    /// thread pool is dropped.
    pub fn run(self) {
        unsafe { main_loop(self.worker, self.registry, self.index) }
    }
}

impl fmt::Debug for ThreadBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ThreadBuilder")
            .field("pool", &self.registry.id())
            .field("index", &self.index)
            .field("name", &self.name)
            .field("stack_size", &self.stack_size)
            .finish()
    }
}

/// Generalized trait for spawning a thread in the `Registry`.
///
/// This trait is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
pub trait ThreadSpawn {
    private_decl! {}

    /// Spawn a thread with the `ThreadBuilder` parameters, and then
    /// call `ThreadBuilder::run()`.
    fn spawn(&mut self, ThreadBuilder) -> io::Result<()>;
}

/// Spawns a thread in the "normal" way with `std::thread::Builder`.
///
/// This type is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
#[derive(Debug, Default)]
pub struct DefaultSpawn;

impl ThreadSpawn for DefaultSpawn {
    private_impl! {}

    fn spawn(&mut self, thread: ThreadBuilder) -> io::Result<()> {
        let mut b = thread::Builder::new();
        if let Some(name) = thread.name() {
            b = b.name(name.to_owned());
        }
        if let Some(stack_size) = thread.stack_size() {
            b = b.stack_size(stack_size);
        }
        b.spawn(|| thread.run())?;
        Ok(())
    }
}

/// Spawns a thread with a user's custom callback.
///
/// This type is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
#[derive(Debug)]
pub struct CustomSpawn<F>(F);

impl<F> CustomSpawn<F>
where
    F: FnMut(ThreadBuilder) -> io::Result<()>,
{
    pub(super) fn new(spawn: F) -> Self {
        CustomSpawn(spawn)
    }
}

impl<F> ThreadSpawn for CustomSpawn<F>
where
    F: FnMut(ThreadBuilder) -> io::Result<()>,
{
    private_impl! {}

    #[inline]
    fn spawn(&mut self, thread: ThreadBuilder) -> io::Result<()> {
        (self.0)(thread)
    }
}

pub(super) struct Registry {
    thread_infos: Vec<ThreadInfo>,
    sleep: Sleep,
    injected_jobs: SegQueue<JobRef>,
    panic_handler: Option<Box<PanicHandler>>,
    start_handler: Option<Box<StartHandler>>,
    exit_handler: Option<Box<ExitHandler>>,

    // When this latch reaches 0, it means that all work on this
    // registry must be complete. This is ensured in the following ways:
    //
    // - if this is the global registry, there is a ref-count that never
    //   gets released.
    // - if this is a user-created thread-pool, then so long as the thread-pool
    //   exists, it holds a reference.
    // - when we inject a "blocking job" into the registry with `ThreadPool::install()`,
    //   no adjustment is needed; the `ThreadPool` holds the reference, and since we won't
    //   return until the blocking job is complete, that ref will continue to be held.
    // - when `join()` or `scope()` is invoked, similarly, no adjustments are needed.
    //   These are always owned by some other job (e.g., one injected by `ThreadPool::install()`)
    //   and that job will keep the pool alive.
    terminate_latch: CountLatch,
}

/// ////////////////////////////////////////////////////////////////////////
/// Initialization

static mut THE_REGISTRY: Option<&'static Arc<Registry>> = None;
static THE_REGISTRY_SET: Once = Once::new();

/// Starts the worker threads (if that has not already happened). If
/// initialization has not already occurred, use the default
/// configuration.
fn global_registry() -> &'static Arc<Registry> {
    set_global_registry(|| Registry::new(ThreadPoolBuilder::new()))
        .or_else(|err| unsafe { THE_REGISTRY.ok_or(err) })
        .expect("The global thread pool has not been initialized.")
}

/// Starts the worker threads (if that has not already happened) with
/// the given builder.
pub(super) fn init_global_registry<S>(
    builder: ThreadPoolBuilder<S>,
) -> Result<&'static Arc<Registry>, ThreadPoolBuildError>
where
    S: ThreadSpawn,
{
    set_global_registry(|| Registry::new(builder))
}

/// Starts the worker threads (if that has not already happened)
/// by creating a registry with the given callback.
fn set_global_registry<F>(registry: F) -> Result<&'static Arc<Registry>, ThreadPoolBuildError>
where
    F: FnOnce() -> Result<Arc<Registry>, ThreadPoolBuildError>,
{
    let mut result = Err(ThreadPoolBuildError::new(
        ErrorKind::GlobalPoolAlreadyInitialized,
    ));
    THE_REGISTRY_SET.call_once(|| {
        result = registry().map(|registry| {
            let registry = leak(registry);
            unsafe {
                THE_REGISTRY = Some(registry);
            }
            registry
        });
    });
    result
}

struct Terminator<'a>(&'a Arc<Registry>);

impl<'a> Drop for Terminator<'a> {
    fn drop(&mut self) {
        self.0.terminate()
    }
}

impl Registry {
    pub(super) fn new<S>(
        mut builder: ThreadPoolBuilder<S>,
    ) -> Result<Arc<Self>, ThreadPoolBuildError>
    where
        S: ThreadSpawn,
    {
        let n_threads = builder.get_num_threads();
        let breadth_first = builder.get_breadth_first();

        let (workers, stealers): (Vec<_>, Vec<_>) = (0..n_threads)
            .map(|_| {
                let worker = if breadth_first {
                    Worker::new_fifo()
                } else {
                    Worker::new_lifo()
                };

                let stealer = worker.stealer();
                (worker, stealer)
            })
            .unzip();

        let registry = Arc::new(Registry {
            thread_infos: stealers.into_iter().map(ThreadInfo::new).collect(),
            sleep: Sleep::new(),
            injected_jobs: SegQueue::new(),
            terminate_latch: CountLatch::new(),
            panic_handler: builder.take_panic_handler(),
            start_handler: builder.take_start_handler(),
            exit_handler: builder.take_exit_handler(),
        });

        // If we return early or panic, make sure to terminate existing threads.
        let t1000 = Terminator(&registry);

        for (index, worker) in workers.into_iter().enumerate() {
            let thread = ThreadBuilder {
                name: builder.get_thread_name(index),
                stack_size: builder.get_stack_size(),
                registry: registry.clone(),
                worker,
                index,
            };
            if let Err(e) = builder.get_spawn_handler().spawn(thread) {
                return Err(ThreadPoolBuildError::new(ErrorKind::IOError(e)));
            }
        }

        // Returning normally now, without termination.
        mem::forget(t1000);

        Ok(registry.clone())
    }

    #[cfg(rayon_unstable)]
    pub(super) fn global() -> Arc<Registry> {
        global_registry().clone()
    }

    pub(super) fn current() -> Arc<Registry> {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                global_registry().clone()
            } else {
                (*worker_thread).registry.clone()
            }
        }
    }

    /// Returns the number of threads in the current registry.  This
    /// is better than `Registry::current().num_threads()` because it
    /// avoids incrementing the `Arc`.
    pub(super) fn current_num_threads() -> usize {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                global_registry().num_threads()
            } else {
                (*worker_thread).registry.num_threads()
            }
        }
    }

    /// Returns the current `WorkerThread` if it's part of this `Registry`.
    pub(super) fn current_thread(&self) -> Option<&WorkerThread> {
        unsafe {
            let worker = WorkerThread::current().as_ref()?;
            if worker.registry().id() == self.id() {
                Some(worker)
            } else {
                None
            }
        }
    }

    /// Returns an opaque identifier for this registry.
    pub(super) fn id(&self) -> RegistryId {
        // We can rely on `self` not to change since we only ever create
        // registries that are boxed up in an `Arc` (see `new()` above).
        RegistryId {
            addr: self as *const Self as usize,
        }
    }

    pub(super) fn num_threads(&self) -> usize {
        self.thread_infos.len()
    }

    pub(super) fn handle_panic(&self, err: Box<dyn Any + Send>) {
        match self.panic_handler {
            Some(ref handler) => {
                // If the customizable panic handler itself panics,
                // then we abort.
                let abort_guard = unwind::AbortIfPanic;
                handler(err);
                mem::forget(abort_guard);
            }
            None => {
                // Default panic handler aborts.
                let _ = unwind::AbortIfPanic; // let this drop.
            }
        }
    }

    /// Waits for the worker threads to get up and running.  This is
    /// meant to be used for benchmarking purposes, primarily, so that
    /// you can get more consistent numbers by having everything
    /// "ready to go".
    pub(super) fn wait_until_primed(&self) {
        for info in &self.thread_infos {
            info.primed.wait();
        }
    }

    /// Waits for the worker threads to stop. This is used for testing
    /// -- so we can check that termination actually works.
    #[cfg(test)]
    pub(super) fn wait_until_stopped(&self) {
        for info in &self.thread_infos {
            info.stopped.wait();
        }
    }

    /// ////////////////////////////////////////////////////////////////////////
    /// MAIN LOOP
    ///
    /// So long as all of the worker threads are hanging out in their
    /// top-level loop, there is no work to be done.

    /// Push a job into the given `registry`. If we are running on a
    /// worker thread for the registry, this will push onto the
    /// deque. Else, it will inject from the outside (which is slower).
    pub(super) fn inject_or_push(&self, job_ref: JobRef) {
        let worker_thread = WorkerThread::current();
        unsafe {
            if !worker_thread.is_null() && (*worker_thread).registry().id() == self.id() {
                (*worker_thread).push(job_ref);
            } else {
                self.inject(&[job_ref]);
            }
        }
    }

    /// Unsafe: the caller must guarantee that `task` will stay valid
    /// until it executes.
    #[cfg(rayon_unstable)]
    pub(super) unsafe fn submit_task<T>(&self, task: Arc<T>)
    where
        T: Task,
    {
        let task_job = TaskJob::new(task);
        let task_job_ref = TaskJob::into_job_ref(task_job);
        return self.inject_or_push(task_job_ref);

        /// A little newtype wrapper for `T`, just because I did not
        /// want to implement `Job` for all `T: Task`.
        struct TaskJob<T: Task> {
            _data: T,
        }

        impl<T: Task> TaskJob<T> {
            fn new(arc: Arc<T>) -> Arc<Self> {
                // `TaskJob<T>` has the same layout as `T`, so we can safely
                // tranmsute this `T` into a `TaskJob<T>`. This lets us write our
                // impls of `Job` for `TaskJob<T>`, making them more restricted.
                // Since `Job` is a private trait, this is not strictly necessary,
                // I don't think, but makes me feel better.
                unsafe { mem::transmute(arc) }
            }

            fn into_task(this: Arc<TaskJob<T>>) -> Arc<T> {
                // Same logic as `new()`
                unsafe { mem::transmute(this) }
            }

            unsafe fn into_job_ref(this: Arc<Self>) -> JobRef {
                let this: *const Self = mem::transmute(this);
                JobRef::new(this)
            }
        }

        impl<T: Task> Job for TaskJob<T> {
            unsafe fn execute(this: *const Self) {
                let this: Arc<Self> = mem::transmute(this);
                let task: Arc<T> = TaskJob::into_task(this);
                Task::execute(task);
            }
        }
    }

    /// Push a job into the "external jobs" queue; it will be taken by
    /// whatever worker has nothing to do. Use this is you know that
    /// you are not on a worker of this registry.
    pub(super) fn inject(&self, injected_jobs: &[JobRef]) {
        log!(InjectJobs {
            count: injected_jobs.len()
        });

        // It should not be possible for `state.terminate` to be true
        // here. It is only set to true when the user creates (and
        // drops) a `ThreadPool`; and, in that case, they cannot be
        // calling `inject()` later, since they dropped their
        // `ThreadPool`.
        assert!(
            !self.terminate_latch.probe(),
            "inject() sees state.terminate as true"
        );

        for &job_ref in injected_jobs {
            self.injected_jobs.push(job_ref);
        }
        self.sleep.tickle(usize::MAX);
    }

    fn pop_injected_job(&self, worker_index: usize) -> Option<JobRef> {
        let job = self.injected_jobs.pop().ok();
        if job.is_some() {
            log!(UninjectedWork {
                worker: worker_index
            });
        }
        job
    }

    /// If already in a worker-thread of this registry, just execute `op`.
    /// Otherwise, inject `op` in this thread-pool. Either way, block until `op`
    /// completes and return its return value. If `op` panics, that panic will
    /// be propagated as well.  The second argument indicates `true` if injection
    /// was performed, `false` if executed directly.
    pub(super) fn in_worker<OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce(&WorkerThread, bool) -> R + Send,
        R: Send,
    {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                self.in_worker_cold(op)
            } else if (*worker_thread).registry().id() != self.id() {
                self.in_worker_cross(&*worker_thread, op)
            } else {
                // Perfectly valid to give them a `&T`: this is the
                // current thread, so we know the data structure won't be
                // invalidated until we return.
                op(&*worker_thread, false)
            }
        }
    }

    #[cold]
    unsafe fn in_worker_cold<OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce(&WorkerThread, bool) -> R + Send,
        R: Send,
    {
        thread_local!(static LOCK_LATCH: LockLatch = LockLatch::new());

        LOCK_LATCH.with(|l| {
            // This thread isn't a member of *any* thread pool, so just block.
            debug_assert!(WorkerThread::current().is_null());
            let job = StackJob::new(
                |injected| {
                    let worker_thread = WorkerThread::current();
                    assert!(injected && !worker_thread.is_null());
                    op(&*worker_thread, true)
                },
                l,
            );
            self.inject(&[job.as_job_ref()]);
            job.latch.wait_and_reset(); // Make sure we can use the same latch again next time.
            job.into_result()
        })
    }

    #[cold]
    unsafe fn in_worker_cross<OP, R>(&self, current_thread: &WorkerThread, op: OP) -> R
    where
        OP: FnOnce(&WorkerThread, bool) -> R + Send,
        R: Send,
    {
        // This thread is a member of a different pool, so let it process
        // other work while waiting for this `op` to complete.
        debug_assert!(current_thread.registry().id() != self.id());
        let latch = TickleLatch::new(SpinLatch::new(), &current_thread.registry().sleep);
        let job = StackJob::new(
            |injected| {
                let worker_thread = WorkerThread::current();
                assert!(injected && !worker_thread.is_null());
                op(&*worker_thread, true)
            },
            latch,
        );
        self.inject(&[job.as_job_ref()]);
        current_thread.wait_until(&job.latch);
        job.into_result()
    }

    /// Increment the terminate counter. This increment should be
    /// balanced by a call to `terminate`, which will decrement. This
    /// is used when spawning asynchronous work, which needs to
    /// prevent the registry from terminating so long as it is active.
    ///
    /// Note that blocking functions such as `join` and `scope` do not
    /// need to concern themselves with this fn; their context is
    /// responsible for ensuring the current thread-pool will not
    /// terminate until they return.
    ///
    /// The global thread-pool always has an outstanding reference
    /// (the initial one). Custom thread-pools have one outstanding
    /// reference that is dropped when the `ThreadPool` is dropped:
    /// since installing the thread-pool blocks until any joins/scopes
    /// complete, this ensures that joins/scopes are covered.
    ///
    /// The exception is `::spawn()`, which can create a job outside
    /// of any blocking scope. In that case, the job itself holds a
    /// terminate count and is responsible for invoking `terminate()`
    /// when finished.
    pub(super) fn increment_terminate_count(&self) {
        self.terminate_latch.increment();
    }

    /// Signals that the thread-pool which owns this registry has been
    /// dropped. The worker threads will gradually terminate, once any
    /// extant work is completed.
    pub(super) fn terminate(&self) {
        self.terminate_latch.set();
        self.sleep.tickle(usize::MAX);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RegistryId {
    addr: usize,
}

struct ThreadInfo {
    /// Latch set once thread has started and we are entering into the
    /// main loop. Used to wait for worker threads to become primed,
    /// primarily of interest for benchmarking.
    primed: LockLatch,

    /// Latch is set once worker thread has completed. Used to wait
    /// until workers have stopped; only used for tests.
    stopped: LockLatch,

    /// the "stealer" half of the worker's deque
    stealer: Stealer<JobRef>,
}

impl ThreadInfo {
    fn new(stealer: Stealer<JobRef>) -> ThreadInfo {
        ThreadInfo {
            primed: LockLatch::new(),
            stopped: LockLatch::new(),
            stealer,
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// WorkerThread identifiers

pub(super) struct WorkerThread {
    /// the "worker" half of our local deque
    worker: Worker<JobRef>,

    /// local queue used for `spawn_fifo` indirection
    fifo: JobFifo,

    index: usize,

    /// A weak random number generator.
    rng: XorShift64Star,

    registry: Arc<Registry>,
}

// This is a bit sketchy, but basically: the WorkerThread is
// allocated on the stack of the worker on entry and stored into this
// thread local variable. So it will remain valid at least until the
// worker is fully unwound. Using an unsafe pointer avoids the need
// for a RefCell<T> etc.
thread_local! {
    static WORKER_THREAD_STATE: Cell<*const WorkerThread> = Cell::new(ptr::null());
}

impl Drop for WorkerThread {
    fn drop(&mut self) {
        // Undo `set_current`
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().eq(&(self as *const _)));
            t.set(ptr::null());
        });
    }
}

impl WorkerThread {
    /// Gets the `WorkerThread` index for the current thread; returns
    /// NULL if this is not a worker thread. This pointer is valid
    /// anywhere on the current thread.
    #[inline]
    pub(super) fn current() -> *const WorkerThread {
        WORKER_THREAD_STATE.with(Cell::get)
    }

    /// Sets `self` as the worker thread index for the current thread.
    /// This is done during worker thread startup.
    unsafe fn set_current(thread: *const WorkerThread) {
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().is_null());
            t.set(thread);
        });
    }

    /// Returns the registry that owns this worker thread.
    pub(super) fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// Our index amongst the worker threads (ranges from `0..self.num_threads()`).
    #[inline]
    pub(super) fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub(super) unsafe fn push(&self, job: JobRef) {
        self.worker.push(job);
        self.registry.sleep.tickle(self.index);
    }

    #[inline]
    pub(super) unsafe fn push_fifo(&self, job: JobRef) {
        self.push(self.fifo.push(job));
    }

    #[inline]
    pub(super) fn local_deque_is_empty(&self) -> bool {
        self.worker.is_empty()
    }

    /// Attempts to obtain a "local" job -- typically this means
    /// popping from the top of the stack, though if we are configured
    /// for breadth-first execution, it would mean dequeuing from the
    /// bottom.
    #[inline]
    pub(super) unsafe fn take_local_job(&self) -> Option<JobRef> {
        self.worker.pop()
    }

    /// Wait until the latch is set. Try to keep busy by popping and
    /// stealing tasks as necessary.
    #[inline]
    pub(super) unsafe fn wait_until<L: LatchProbe + ?Sized>(&self, latch: &L) {
        log!(WaitUntil { worker: self.index });
        if !latch.probe() {
            self.wait_until_cold(latch);
        }
    }

    #[cold]
    unsafe fn wait_until_cold<L: LatchProbe + ?Sized>(&self, latch: &L) {
        // the code below should swallow all panics and hence never
        // unwind; but if something does wrong, we want to abort,
        // because otherwise other code in rayon may assume that the
        // latch has been signaled, and that can lead to random memory
        // accesses, which would be *very bad*
        let abort_guard = unwind::AbortIfPanic;

        let mut yields = 0;
        while !latch.probe() {
            // Try to find some work to do. We give preference first
            // to things in our local deque, then in other workers
            // deques, and finally to injected jobs from the
            // outside. The idea is to finish what we started before
            // we take on something new.
            if let Some(job) = self
                .take_local_job()
                .or_else(|| self.steal())
                .or_else(|| self.registry.pop_injected_job(self.index))
            {
                yields = self.registry.sleep.work_found(self.index, yields);
                self.execute(job);
            } else {
                yields = self.registry.sleep.no_work_found(self.index, yields);
            }
        }

        // If we were sleepy, we are not anymore. We "found work" --
        // whatever the surrounding thread was doing before it had to
        // wait.
        self.registry.sleep.work_found(self.index, yields);

        log!(LatchSet { worker: self.index });
        mem::forget(abort_guard); // successful execution, do not abort
    }

    pub(super) unsafe fn execute(&self, job: JobRef) {
        job.execute();

        // Subtle: executing this job will have `set()` some of its
        // latches.  This may mean that a sleepy (or sleeping) worker
        // can now make progress. So we have to tickle them to let
        // them know.
        self.registry.sleep.tickle(self.index);
    }

    /// Try to steal a single job and return it.
    ///
    /// This should only be done as a last resort, when there is no
    /// local work to do.
    unsafe fn steal(&self) -> Option<JobRef> {
        // we only steal when we don't have any work to do locally
        debug_assert!(self.local_deque_is_empty());

        // otherwise, try to steal
        let num_threads = self.registry.thread_infos.len();
        if num_threads <= 1 {
            return None;
        }

        let start = self.rng.next_usize(num_threads);
        (start..num_threads)
            .chain(0..start)
            .filter(|&i| i != self.index)
            .filter_map(|victim_index| {
                let victim = &self.registry.thread_infos[victim_index];
                loop {
                    match victim.stealer.steal() {
                        Steal::Empty => return None,
                        Steal::Success(d) => {
                            log!(StoleWork {
                                worker: self.index,
                                victim: victim_index
                            });
                            return Some(d);
                        }
                        Steal::Retry => {}
                    }
                }
            })
            .next()
    }
}

/// ////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(worker: Worker<JobRef>, registry: Arc<Registry>, index: usize) {
    let worker_thread = &WorkerThread {
        worker,
        fifo: JobFifo::new(),
        index,
        rng: XorShift64Star::new(),
        registry: registry.clone(),
    };
    WorkerThread::set_current(worker_thread);

    // let registry know we are ready to do work
    registry.thread_infos[index].primed.set();

    // Worker threads should not panic. If they do, just abort, as the
    // internal state of the threadpool is corrupted. Note that if
    // **user code** panics, we should catch that and redirect.
    let abort_guard = unwind::AbortIfPanic;

    // Inform a user callback that we started a thread.
    if let Some(ref handler) = registry.start_handler {
        let registry = registry.clone();
        match unwind::halt_unwinding(|| handler(index)) {
            Ok(()) => {}
            Err(err) => {
                registry.handle_panic(err);
            }
        }
    }

    worker_thread.wait_until(&registry.terminate_latch);

    // Should not be any work left in our queue.
    debug_assert!(worker_thread.take_local_job().is_none());

    // let registry know we are done
    registry.thread_infos[index].stopped.set();

    // Normal termination, do not abort.
    mem::forget(abort_guard);

    // Inform a user callback that we exited a thread.
    if let Some(ref handler) = registry.exit_handler {
        let registry = registry.clone();
        match unwind::halt_unwinding(|| handler(index)) {
            Ok(()) => {}
            Err(err) => {
                registry.handle_panic(err);
            }
        }
        // We're already exiting the thread, there's nothing else to do.
    }
}

/// If already in a worker-thread, just execute `op`.  Otherwise,
/// execute `op` in the default thread-pool. Either way, block until
/// `op` completes and return its return value. If `op` panics, that
/// panic will be propagated as well.  The second argument indicates
/// `true` if injection was performed, `false` if executed directly.
pub(super) fn in_worker<OP, R>(op: OP) -> R
where
    OP: FnOnce(&WorkerThread, bool) -> R + Send,
    R: Send,
{
    unsafe {
        let owner_thread = WorkerThread::current();
        if !owner_thread.is_null() {
            // Perfectly valid to give them a `&T`: this is the
            // current thread, so we know the data structure won't be
            // invalidated until we return.
            op(&*owner_thread, false)
        } else {
            global_registry().in_worker_cold(op)
        }
    }
}

/// [xorshift*] is a fast pseudorandom number generator which will
/// even tolerate weak seeding, as long as it's not zero.
///
/// [xorshift*]: https://en.wikipedia.org/wiki/Xorshift#xorshift*
struct XorShift64Star {
    state: Cell<u64>,
}

impl XorShift64Star {
    fn new() -> Self {
        // Any non-zero seed will do -- this uses the hash of a global counter.
        let mut seed = 0;
        while seed == 0 {
            let mut hasher = DefaultHasher::new();
            #[allow(deprecated)]
            static COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;
            hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
            seed = hasher.finish();
        }

        XorShift64Star {
            state: Cell::new(seed),
        }
    }

    fn next(&self) -> u64 {
        let mut x = self.state.get();
        debug_assert_ne!(x, 0);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state.set(x);
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Return a value from `0..n`.
    fn next_usize(&self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}
