use ::{Configuration, ExitHandler, PanicHandler, StartHandler, MainHandler};
use coco::deque::{self, Worker, Stealer};
use job::{JobRef, StackJob};
#[cfg(rayon_unstable)]
use job::Job;
#[cfg(rayon_unstable)]
use internal::task::Task;
use latch::{LatchProbe, Latch, CountLatch, LockLatch}; // , SpinLatch, TickleLatch
use log::Event::*;
use rand::{self, Rng};
use sleep::Sleep;
use std::any::Any;
use std::error::Error;
use std::cell::{RefCell, Cell, UnsafeCell};
use parking_lot::Mutex;
use std::sync::{Arc, Once, ONCE_INIT};
use std::thread;
use std::mem;
use std::fmt;
use std::u32;
use std::usize;
use unwind;
use util::leak;
use crossbeam_channel;
use fiber::{FiberStack, Fiber, ResumeAction, Waitable, TransferInfo};
#[cfg(feature = "debug")]
use fiber;
#[cfg(feature = "debug")]
use std::collections::HashSet;
#[cfg(feature = "debug")]
use fiber::{Ptr, FiberId, FiberAction};
#[cfg(feature = "debug")]
use ctrlc;
#[cfg(feature = "debug")]
use std::collections::VecDeque;
#[cfg(feature = "debug")]
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
#[cfg(feature = "debug")]
use std::sync::Weak;
#[cfg(windows)]
use std::os::windows::io::IntoRawHandle;
#[cfg(windows)]
use winapi;
#[cfg(windows)]
use kernel32;

/// Error if the gloal thread pool is initialized multiple times.
#[derive(Debug,PartialEq)]
struct GlobalPoolAlreadyInitialized;

impl fmt::Display for GlobalPoolAlreadyInitialized {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl Error for GlobalPoolAlreadyInitialized {
    fn description(&self) -> &str {
        "The global thread pool has already been initialized."
    }
}

#[cfg(windows)]
pub struct ThreadHandle(pub winapi::HANDLE);
#[cfg(windows)]
unsafe impl Send for ThreadHandle {}
#[cfg(windows)]
unsafe impl Sync for ThreadHandle {}

pub struct Registry {
    thread_infos: Vec<ThreadInfo>,
    job_injector: crossbeam_channel::Sender<JobRef>,
    pub sleep: Sleep,
    job_uninjector: crossbeam_channel::Receiver<JobRef>,
    panic_handler: Option<Box<PanicHandler>>,
    start_handler: Option<Box<StartHandler>>,
    main_handler: Option<Box<MainHandler>>,
    exit_handler: Option<Box<ExitHandler>>,
    pub stack_size: usize,

    #[cfg(feature = "debug")]
    workers: Mutex<HashSet<usize>>,
    #[cfg(feature = "debug")]
    pub active_fibers: AtomicUsize,
    
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

    #[cfg(windows)]
    pub handles: Mutex<Vec<ThreadHandle>>,
}

scoped_thread_local!(static CURRENT_REGISTRY: Arc<Registry>);

pub fn set_current_registry<F: FnOnce() -> R, R>(registry: &Arc<Registry>, f: F) -> R {
    CURRENT_REGISTRY.set(registry, f)
}

/// Starts the worker threads (if that has not already happened). If
/// initialization has not already occurred, use the default
/// configuration.
fn with_current_registry<F: FnOnce(&Arc<Registry>) -> R, R> (f: F) -> R {
    if CURRENT_REGISTRY.is_set() {
        CURRENT_REGISTRY.with(f)
    } else {
        f(global_registry())
    }
}

pub fn get_current_registry() -> Option<Arc<Registry>> {
    if CURRENT_REGISTRY.is_set() {
        CURRENT_REGISTRY.with(|r| Some(r.clone()))
    } else {
        None
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Initialization

static mut THE_REGISTRY: Option<&'static Arc<Registry>> = None;
static THE_REGISTRY_SET: Once = ONCE_INIT;

/// Starts the worker threads (if that has not already happened). If
/// initialization has not already occurred, use the default
/// configuration.
fn global_registry() -> &'static Arc<Registry> {
    THE_REGISTRY_SET.call_once(|| unsafe { init_registry(Configuration::new().num_threads(4)).unwrap() });
    unsafe { THE_REGISTRY.expect("The global thread pool has not been initialized.") }
}

/// Starts the worker threads (if that has not already happened) with
/// the given configuration.
pub fn init_global_registry(config: Configuration) -> Result<&'static Registry, Box<Error>> {
    let mut called = false;
    let mut init_result = Ok(());;
    THE_REGISTRY_SET.call_once(|| unsafe {
        init_result = init_registry(config);
        called = true;
    });
    if called {
        init_result.map(|()| &**global_registry())
    } else {
        Err(Box::new(GlobalPoolAlreadyInitialized))
    }
}

/// Initializes the global registry with the given configuration.
/// Meant to be called from within the `THE_REGISTRY_SET` once
/// function. Declared `unsafe` because it writes to `THE_REGISTRY` in
/// an unsynchronized fashion.
unsafe fn init_registry(config: Configuration) -> Result<(), Box<Error>> {
    Registry::new(config).map(|registry| THE_REGISTRY = Some(leak(registry)))
}

struct Terminator<'a>(&'a Arc<Registry>);

impl<'a> Drop for Terminator<'a> {
    fn drop(&mut self) {
        self.0.terminate()
    }
}

#[cfg(windows)]
impl Drop for Registry {
    fn drop(&mut self) {
        for handle in self.handles.get_mut() {
            unsafe {
                kernel32::CloseHandle(handle.0);
            }
        }
    }
}

impl Registry {
    pub fn new(mut configuration: Configuration) -> Result<Arc<Registry>, Box<Error>> {
        let n_threads = configuration.get_num_threads();
        let breadth_first = configuration.get_breadth_first();

        #[cfg(feature = "debug")]
        ctrlc::set_handler(move || {
            use std;
            ctrlc();
            fiber::ctrlc();
            println!("saw ctrl-c");
            std::process::abort();
        }).ok();

        let (job_injector, job_uninjector) = crossbeam_channel::unbounded();
        
        let (workers, stealers): (Vec<_>, Vec<_>) = (0..n_threads).map(|_| deque::new()).unzip();
        let (tx_ready_tasks, rx_resumed_tasks): (Vec<_>, Vec<_>) = {
            (0..n_threads).map(|_| crossbeam_channel::unbounded()).unzip()
        };

        let registry = Arc::new(Registry {
            thread_infos: stealers.into_iter().zip(tx_ready_tasks.into_iter())
                .map(|(s, r)| ThreadInfo::new(s, r))
                .collect(),
            job_injector,
            job_uninjector,
            sleep: Sleep::new(),
            #[cfg(feature = "debug")]
            workers: Mutex::new(HashSet::new()),
            #[cfg(feature = "debug")]
            active_fibers: AtomicUsize::new(0),
            terminate_latch: CountLatch::new(),
            panic_handler: configuration.take_panic_handler(),
            start_handler: configuration.take_start_handler(),
            main_handler: configuration.take_main_handler(),
            exit_handler: configuration.take_exit_handler(),
            stack_size: configuration.get_stack_size().unwrap_or(1024 * 1024),
            #[cfg(windows)]
            handles: Mutex::new(Vec::new()),
        });

        #[cfg(feature = "debug")]
        REGISTRIES.lock().push(Arc::downgrade(&registry));

        // If we return early or panic, make sure to terminate existing threads.
        let t1000 = Terminator(&registry);

        let iter = workers.into_iter().zip(rx_resumed_tasks.into_iter()).enumerate();

        #[cfg(windows)]
        let mut handles = Vec::new();

        for (index, (worker, rx_resumed_tasks)) in iter {
            let registry = registry.clone();
            let mut b = thread::Builder::new();
            if let Some(name) = configuration.get_thread_name(index) {
                b = b.name(name);
            }
            if let Some(stack_size) = configuration.get_stack_size() {
                b = b.stack_size(stack_size);
            }
            let _handle = try!(b.spawn(move || unsafe {
                main_loop(worker, rx_resumed_tasks, registry, index, breadth_first)
            }));
            #[cfg(windows)]
            handles.push(ThreadHandle(_handle.into_raw_handle()));
        }

        #[cfg(windows)]
        {
            *registry.handles.lock() = handles;
        }

        // Returning normally now, without termination.
        mem::forget(t1000);

        Ok(registry.clone())
    }

    #[cfg(rayon_unstable)]
    pub fn global() -> Arc<Registry> {
        with_current_registry(|r| r.clone())
    }

    pub fn signal(&self) {
        self.sleep.tickle(usize::MAX);
    }

    pub fn current() -> Arc<Registry> {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                with_current_registry(|r| r.clone())
            } else {
                (*worker_thread).registry.clone()
            }
        }
    }

    /// Returns the number of threads in the current registry.  This
    /// is better than `Registry::current().num_threads()` because it
    /// avoids incrementing the `Arc`.
    pub fn current_num_threads() -> usize {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                with_current_registry(|r| r.num_threads())
            } else {
                (*worker_thread).registry.num_threads()
            }
        }
    }


    /// Returns an opaque identifier for this registry.
    pub fn id(&self) -> RegistryId {
        // We can rely on `self` not to change since we only ever create
        // registries that are boxed up in an `Arc` (see `new()` above).
        RegistryId { addr: self as *const Self as usize }
    }

    pub fn num_threads(&self) -> usize {
        self.thread_infos.len()
    }

    pub fn handle_panic(&self, err: Box<Any + Send>) {
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

    pub fn resume_fiber(&self, worker_index: usize, fiber: Fiber) {
        let sender = &self.thread_infos[worker_index].tx_ready_tasks;
        sender.send(fiber).unwrap();
    }

    /// Waits for the worker threads to get up and running.  This is
    /// meant to be used for benchmarking purposes, primarily, so that
    /// you can get more consistent numbers by having everything
    /// "ready to go".
    pub fn wait_until_primed(&self) {
        for info in &self.thread_infos {
            info.primed.wait();
        }
    }

    /// Waits for the worker threads to stop. This is used for testing
    /// -- so we can check that termination actually works.
    pub fn wait_until_stopped(&self) {
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
    pub fn inject_or_push(&self, job_ref: JobRef) {
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
    pub unsafe fn submit_task<T>(&self, task: Arc<T>)
        where T: Task
    {
        let task_job = TaskJob::new(task);
        let task_job_ref = TaskJob::into_job_ref(task_job);
        return self.inject_or_push(task_job_ref);

        /// A little newtype wrapper for `T`, just because I did not
        /// want to implement `Job` for all `T: Task`.
        struct TaskJob<T: Task> {
            _data: T
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

            pub fn into_task(this: Arc<TaskJob<T>>) -> Arc<T> {
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
    pub fn inject(&self, injected_jobs: &[JobRef]) {
        log!(InjectJobs { count: injected_jobs.len() });
        {
            // It should not be possible for `state.terminate` to be true
            // here. It is only set to true when the user creates (and
            // drops) a `ThreadPool`; and, in that case, they cannot be
            // calling `inject()` later, since they dropped their
            // `ThreadPool`.
            assert!(!self.terminate_latch.probe(), "inject() sees state.terminate as true");

            for &job_ref in injected_jobs {
                self.job_injector.send(job_ref).unwrap();
            }
        }
        self.sleep.tickle(usize::MAX);
    }

    fn pop_injected_job(&self, worker_index: usize) -> Option<JobRef> {
        let stolen = self.job_uninjector.try_recv().ok();
        if stolen.is_some() {
            log!(UninjectedWork { worker: worker_index });
        }
        stolen
    }

    /// If already in a worker-thread of this registry, just execute `op`.
    /// Otherwise, inject `op` in this thread-pool. Either way, block until `op`
    /// completes and return its return value. If `op` panics, that panic will
    /// be propagated as well.  The second argument indicates `true` if injection
    /// was performed, `false` if executed directly.
    pub fn in_worker<OP, R>(&self, op: OP) -> R
        where OP: FnOnce(&WorkerThread, bool) -> R + Send, R: Send
    {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                self.in_worker_cold(op)
            //} else if (*worker_thread).registry().id() != self.id() {
            //    self.in_worker_cross(&*worker_thread, op)
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
        where OP: FnOnce(&WorkerThread, bool) -> R + Send, R: Send
    {
        // This thread isn't a member of *any* thread pool, so just block.
        debug_assert!(WorkerThread::current().is_null());
        let job = StackJob::new(|injected| {
            let worker_thread = WorkerThread::current();
            assert!(injected && !worker_thread.is_null());
            op(&*worker_thread, true)
        }, LockLatch::new());
        self.inject(&[job.as_job_ref()]);
        job.latch.wait();
        job.into_result()
    }
/*
    #[cold]
    unsafe fn in_worker_cross<OP, R>(&self, current_thread: &WorkerThread, op: OP) -> R
        where OP: FnOnce(&WorkerThread, bool) -> R + Send, R: Send
    {
        // This thread is a member of a different pool, so let it process
        // other work while waiting for this `op` to complete.
        debug_assert!(current_thread.registry().id() != self.id());
        let latch = TickleLatch::new(SpinLatch::new(), &current_thread.registry().sleep);
        let job = StackJob::new(|injected| {
            let worker_thread = WorkerThread::current();
            assert!(injected && !worker_thread.is_null());
            op(&*worker_thread, true)
        }, latch);
        self.inject(&[job.as_job_ref()]);
        current_thread.wait_until(&job.latch);
        job.into_result()
    }
*/
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
    pub fn increment_terminate_count(&self) {
        self.terminate_latch.increment();
    }

    /// Signals that the thread-pool which owns this registry has been
    /// dropped. The worker threads will gradually terminate, once any
    /// extant work is completed.
    pub fn terminate(&self) {
        self.terminate_latch.set();
        self.sleep.tickle(usize::MAX);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegistryId {
    addr: usize
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

    /// submit tasks which were waiting on other things, but are now ready to run
    tx_ready_tasks: crossbeam_channel::Sender<Fiber>,
}

impl ThreadInfo {
    fn new(stealer: Stealer<JobRef>, tx_ready_tasks: crossbeam_channel::Sender<Fiber>) -> ThreadInfo {
        ThreadInfo {
            primed: LockLatch::new(),
            stopped: LockLatch::new(),
            stealer: stealer,
            tx_ready_tasks,
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// WorkerThread identifiers

pub struct WorkerThread {
    /// the "worker" half of our local deque
    worker: Worker<JobRef>,

    rx_resumed_tasks: crossbeam_channel::Receiver<Fiber>,

    index: usize,

    #[cfg(feature = "debug")]
    stack_low: Ptr,
    #[cfg(feature = "debug")]
    stack_high: Ptr,

    #[cfg(feature = "debug")]
    pub asleep: AtomicBool,

    /// are these workers configured to steal breadth-first or not?
    breadth_first: bool,

    /// A weak random number generator.
    rng: UnsafeCell<rand::XorShiftRng>,

    pub registry: Arc<Registry>,

    main_loop_fiber: Cell<Option<Fiber>>,

    pub stack_cache: RefCell<Vec<FiberStack>>,

    #[cfg(feature = "debug")]
    pub poisoned_stacks: RefCell<VecDeque<fiber::FiberStack>>,

    #[cfg(feature = "debug")]
    pub current_fiber: RefCell<(Option<Arc<fiber::FiberInfo>>, Option<Arc<fiber::FiberInfo>>)>,
}

// This is a bit sketchy, but basically: the WorkerThread is
// allocated on the stack of the worker on entry and stored into this
// thread local variable. So it will remain valid at least until the
// worker is fully unwound. Using an unsafe pointer avoids the need
// for a RefCell<T> etc.
thread_local! {
    static WORKER_THREAD_STATE: Cell<*const WorkerThread> =
        Cell::new(0 as *const WorkerThread)
}

impl WorkerThread {
    /// Gets the `WorkerThread` index for the current thread; returns
    /// NULL if this is not a worker thread. This pointer is valid
    /// anywhere on the current thread.
    #[inline]
    pub fn current() -> *const WorkerThread {
        WORKER_THREAD_STATE.with(|t| t.get())
    }

    #[cfg(feature = "debug")]
    pub fn fiber_info(&self) -> Arc<fiber::FiberInfo> {
        self.current_fiber.borrow().0.clone().unwrap()
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
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// Our index amongst the worker threads (ranges from `0..self.num_threads()`).
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub unsafe fn push(&self, job: JobRef) {
        self.worker.push(job);
        self.registry.sleep.tickle(self.index);
    }

    #[inline]
    pub fn local_deque_is_empty(&self) -> bool {
        self.worker.len() == 0
    }

    /// Attempts to obtain a "local" job -- typically this means
    /// popping from the top of the stack, though if we are configured
    /// for breadth-first execution, it would mean dequeuing from the
    /// bottom.
    #[inline]
    pub unsafe fn take_local_job(&self) -> Option<JobRef> {
        if !self.breadth_first {
            self.worker.pop()
        } else {
            self.worker.steal()
        }
    }

    #[inline]
    pub unsafe fn take_resumed_job(&self) -> Option<Fiber> {
        self.rx_resumed_tasks.try_recv().ok()
    }

    /// Wait until the latch is set. Try to keep busy by popping and
    /// stealing tasks as necessary.
    #[inline]
    pub unsafe fn wait_until<L: Waitable>(&self, latch: &L) {
        log!(WaitUntil { worker: self.index });
        if !latch.complete(self) {
            self.wait_until_cold(latch);
        }
    }

    #[cold]
    pub unsafe fn find_work<'a>(&self, action: ResumeAction) -> TransferInfo {
        // the code below should swallow all panics and hence never
        // unwind; but if something does wrong, we want to abort,
        // because otherwise other code in rayon may assume that the
        // latch has been signaled, and that can lead to random memory
        // accesses, which would be *very bad*
        let abort_guard = unwind::AbortIfPanic;

        let result;

        // Try to find some work to do. We give preference first
        // to things in our local deque, then in other workers
        // deques, and finally to injected jobs from the
        // outside. The idea is to finish what we started before
        // we take on something new.
        if let Some(fiber) = self.take_resumed_job() {
            fiber_log!("worked {} picked up fiber {} in find_work", self.index(), fiber);
            result = fiber.resume(self, action);
        } else if let Some(job) = self.take_local_job()
                                .or_else(|| self.steal())
                                .or_else(|| self.registry.pop_injected_job(self.index)) {
            result = Fiber::spawn(self, job, action);
        } else {
            // Nothing to do, enter the main loop
            let main_fiber = self.main_loop_fiber.replace(None).unwrap();
            result = main_fiber.resume(self, action);
        }

        mem::forget(abort_guard);

        result
    }

    #[cold]
    unsafe fn wait_until_cold<'l, L: Waitable>(&self, latch: &'l L) {
        // the code below should swallow all panics and hence never
        // unwind; but if something does wrong, we want to abort,
        // because otherwise other code in rayon may assume that the
        // latch has been signaled, and that can lead to random memory
        // accesses, which would be *very bad*
        let abort_guard = unwind::AbortIfPanic;

        let waitable: &'l Waitable = latch;
        let waitable: *const Waitable = mem::transmute(waitable);

        #[cfg(feature = "debug")]
        let fiber_info = self.fiber_info();
        #[cfg(feature = "debug")]
        {
            *fiber_info.action.lock() = FiberAction::WaitUntil(Ptr(latch as *const _ as usize));
        }

        let mut yields = 0;
        while !latch.complete(self) {
            #[cfg(feature = "debug")]
            {
                if let FiberId::Job(..) = fiber_info.id {
                    use std::intrinsics;
                    fiber_log!("latch not complete {:x}, type {}", latch as *const _ as usize, intrinsics::type_name::<L>());
                }
            }
            // Try to find some work to do. We give preference first
            // to things in our local deque, then in other workers
            // deques, and finally to injected jobs from the
            // outside. The idea is to finish what we started before
            // we take on something new.
            if let Some(fiber) = self.take_resumed_job() {
                yields = self.registry.sleep.work_found(self.index, yields);
                fiber_log!("worked {} picked up fiber {} in wait_until_cold", self.index(), fiber);
                fiber.resume(self, ResumeAction::StoreInWaitable(waitable)).handle(self);

                // Subtle: resuming this job will have `set()` some of its
                // latches.  This may mean that a sleepy (or sleeping) worker
                // can now make progress. So we have to tickle them to let
                // them know.
                // CHECK: Find out when this is needed
                self.registry.sleep.tickle(self.index);
                break; // If we resume again, the latch is set
            } else if let Some(job) = self.take_local_job()
                                   .or_else(|| self.steal())
                                   .or_else(|| self.registry.pop_injected_job(self.index)) {
                yields = self.registry.sleep.work_found(self.index, yields);
                self.execute(job, ResumeAction::StoreInWaitable(waitable));
                break; // If we resume again, the latch is set
            } else {
                yields = self.registry.sleep.no_work_found(self, self.index, yields);
            }
        }

        #[cfg(feature = "debug")]
        {
            *fiber_info.action.lock() = FiberAction::Working;
        }

        // If we were sleepy, we are not anymore. We "found work" --
        // whatever the surrounding thread was doing before it had to
        // wait.
        self.registry.sleep.work_found(self.index, yields);

        log!(LatchSet { worker: self.index });
        mem::forget(abort_guard); // successful execution, do not abort
    }

    pub unsafe fn execute(&self, job: JobRef, action: ResumeAction) {
        Fiber::spawn(self, job, action).handle(self);

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
        debug_assert!(self.worker.pop().is_none());

        // otherwise, try to steal
        let num_threads = self.registry.thread_infos.len();
        if num_threads <= 1 {
            return None;
        }
        assert!(num_threads < (u32::MAX as usize),
                "we do not support more than u32::MAX worker threads");

        let start = {
            // OK to use this UnsafeCell because (a) this data is
            // confined to current thread, as WorkerThread is not Send
            // nor Sync and (b) rand crate will not call back into
            // this method.
            let rng = &mut *self.rng.get();
            rng.next_u32() % num_threads as u32
        } as usize;
        (start .. num_threads)
            .chain(0 .. start)
            .filter(|&i| i != self.index)
            .filter_map(|victim_index| {
                let victim = &self.registry.thread_infos[victim_index];
                let stolen = victim.stealer.steal();
                if stolen.is_some() {
                    log!(StoleWork { worker: self.index, victim: victim_index });
                }
                stolen
            })
            .next()
    }
}

/// ////////////////////////////////////////////////////////////////////////

#[cfg(feature = "debug")]
lazy_static! {
    pub static ref REGISTRIES: Mutex<Vec<Weak<Registry>>> = {
        Mutex::new(Vec::new())
    };
}

#[cfg(feature = "debug")]
pub fn is_deadlocked(registry: &Registry) -> bool {
    if registry.active_fibers.load(Ordering::SeqCst) == 0 {
        return false;
    }

    for wt in &*registry.workers.lock() {
        let worker = unsafe { &*(*wt as *const WorkerThread) };
        if !worker.asleep.load(Ordering::SeqCst) {
            return false
        }
    }

    return registry.active_fibers.load(Ordering::SeqCst) > 0;
}

#[cfg(feature = "debug")]
pub fn deadlock_check(registry: &Registry) {
    if is_deadlocked(registry) {
        debug_log!("\n\n ------ RAYON DEADLOCK FOUND ------");
        loop {
            ctrlc();
            fiber::ctrlc();
            #[cfg(windows)]
            unsafe { ::kernel32::DebugBreak() };
            #[cfg(not(windows))]
            unsafe { asm!("ud2" ::::"volatile") };
        }
    }
}

#[cfg(feature = "debug")]
pub fn ctrlc() {
    debug_log!("------ registries ------");
    for r in &*(REGISTRIES.lock()) {
        let registry = if let Some(r) = r.upgrade() {
            r
        } else {
            continue;
        };

        debug_log!("------ registry {:x} ------", &*registry as *const _ as usize);
        debug_log!("  terminate_latch: {:?}", registry.terminate_latch.probe());

        for wt in &*registry.workers.lock() {
            print_worker(*wt);
        }

        debug_log!("------ end registry {:x} ------", &*registry as *const _ as usize);
    }
    debug_log!("------ end registries ------");
}

#[cfg(feature = "debug")]
pub fn print_worker(wt: usize) {
    unsafe {
        let worker = &*(wt as *const WorkerThread);
        debug_log!("worker {:x} - {}", wt, worker.index());
        debug_log!("  stack_low: {:?}", worker.stack_low);
        debug_log!("  stack_high: {:?}", worker.stack_high);
        debug_log!("  asleep = {:?}", worker.asleep.load(Ordering::SeqCst));
        debug_log!("  running fiber {:?}", worker.current_fiber.borrow().0);
        let resumed_job = worker.take_resumed_job();
        debug_log!("  take_resumed_job {:?}", resumed_job);
        mem::forget(resumed_job);
        debug_log!("  take_local_job {:?}", worker.take_local_job());
        debug_log!("  pop_injected_job {:?}", worker.registry.pop_injected_job(worker.index));
    }
}

unsafe fn main_loop(worker: Worker<JobRef>,
                    rx_resumed_tasks: crossbeam_channel::Receiver<Fiber>,
                    registry: Arc<Registry>,
                    index: usize,
                    breadth_first: bool) {
    #[cfg(windows)]
    {
        use std::io;
        use kernel32;

        if kernel32::ConvertThreadToFiber(0i32 as _) == 0i32 as _ {
            panic!("unable to convert worker thread to fiber {}", io::Error::last_os_error());
        }
    }

    #[cfg(feature = "debug")]
    use std::sync::Arc;
/*    use libc;

    let mut attr: libc::pthread_attr_t = mem::zeroed();
    assert_eq!(libc::pthread_attr_init(&mut attr), 0);
    assert_eq!(libc::pthread_getattr_np(libc::pthread_self(),
                                        &mut attr), 0);
    let mut stackaddr = 0 as *mut _;
    let mut stacksize = 0;
    assert_eq!(libc::pthread_attr_getstack(&attr, &mut stackaddr,
                                            &mut stacksize), 0);
    assert_eq!(libc::pthread_attr_destroy(&mut attr), 0);
    let stack_low = stackaddr as usize;
    let stack_high = stack_low + stacksize;
*/

    #[cfg(feature = "debug")]
    let fiber_data = {
        let mut stack_low = 0;
        let mut stack_high = 0;
        #[cfg(windows)] {
            ::kernel32::GetCurrentThreadStackLimits(&mut stack_low, &mut stack_high)
        }
        Arc::new(fiber::FiberInfo::new(FiberId::Worker(index), stack_low as usize, stack_high as usize))
    };
    #[cfg(feature = "debug")]
    let stack_low = *fiber_data.stack_low.lock();
    #[cfg(feature = "debug")]
    let stack_high = *fiber_data.stack_high.lock();
    let worker_thread = WorkerThread {
        worker: worker,
        rx_resumed_tasks,
        breadth_first: breadth_first,
        index: index,
        #[cfg(feature = "debug")]
        stack_low,
        #[cfg(feature = "debug")]
        stack_high,
        #[cfg(feature = "debug")]
        asleep: AtomicBool::new(false),
        rng: UnsafeCell::new(rand::weak_rng()),
        registry: registry.clone(),
        main_loop_fiber: Cell::new(None),
        stack_cache: RefCell::new(Vec::new()),
        #[cfg(feature = "debug")]
        poisoned_stacks: RefCell::new(VecDeque::new()),
        #[cfg(feature = "debug")]
        current_fiber: RefCell::new((Some(fiber_data), None)),
    };

    #[cfg(feature = "debug")]
    registry.workers.lock().insert(&worker_thread as *const _ as usize);

    WorkerThread::set_current(&worker_thread);

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
            Ok(()) => {
            }
            Err(err) => {
                registry.handle_panic(err);
            }
        }
    }

    struct MainLoopWaiter;

    impl Waitable for MainLoopWaiter {
        fn complete(&self, worker_thread: &WorkerThread) -> bool {
            worker_thread.registry.terminate_latch.probe()
        }
        fn await(&self, worker_thread: &WorkerThread, waiter: Fiber) {
            worker_thread.main_loop_fiber.set(Some(waiter));
        }
    }

    let mut work = || {
        while !registry.terminate_latch.probe() {
            worker_thread.wait_until(&MainLoopWaiter);
        }
    };

    if let Some(ref handler) = registry.main_handler {
        let registry = registry.clone();
        match unwind::halt_unwinding(|| handler(index, &mut work)) {
            Ok(()) => {
            }
            Err(err) => {
                registry.handle_panic(err);
            }
        }
    } else {
        work();
    }

    // Should not be any work left in our queue.
    debug_assert!(worker_thread.take_local_job().is_none());

    // let registry know we are done
    registry.thread_infos[index].stopped.set();

    #[cfg(feature = "debug")]
    registry.workers.lock().remove(&(&worker_thread as *const _ as usize));

    // Normal termination, do not abort.
    mem::forget(abort_guard);

    // Inform a user callback that we exited a thread.
    if let Some(ref handler) = registry.exit_handler {
        let registry = registry.clone();
        match unwind::halt_unwinding(|| handler(index)) {
            Ok(()) => {
            }
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
pub fn in_worker<OP, R>(op: OP) -> R
    where OP: FnOnce(&WorkerThread, bool) -> R + Send, R: Send
{
    unsafe {
        let owner_thread = WorkerThread::current();
        if !owner_thread.is_null() {
            // Perfectly valid to give them a `&T`: this is the
            // current thread, so we know the data structure won't be
            // invalidated until we return.
            op(&*owner_thread, false)
        } else {
            with_current_registry(|r| r.in_worker_cold(op))
        }
    }
}
