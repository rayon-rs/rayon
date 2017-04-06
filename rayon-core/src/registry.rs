use ::{Configuration, ExitHandler, PanicHandler, StartHandler};
use deque;
use deque::{Worker, Stealer, Stolen};
use job::{JobRef, StackJob};
use latch::{LatchProbe, Latch, CountLatch, LockLatch};
#[allow(unused_imports)]
use log::Event::*;
use rand::{self, Rng};
use sleep::Sleep;
use std::any::Any;
use std::error::Error;
use std::cell::{Cell, UnsafeCell};
use std::sync::{Arc, Mutex, Once, ONCE_INIT};
use std::thread;
use std::mem;
use std::fmt;
use std::u32;
use std::usize;
use unwind;
use util::leak;

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

pub struct Registry {
    thread_infos: Vec<ThreadInfo>,
    state: Mutex<RegistryState>,
    sleep: Sleep,
    job_uninjector: Stealer<JobRef>,
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

struct RegistryState {
    job_injector: Worker<JobRef>,
}

/// ////////////////////////////////////////////////////////////////////////
/// Initialization

static mut THE_REGISTRY: Option<&'static Arc<Registry>> = None;
static THE_REGISTRY_SET: Once = ONCE_INIT;

/// Starts the worker threads (if that has not already happened). If
/// initialization has not already occurred, use the default
/// configuration.
fn global_registry() -> &'static Arc<Registry> {
    THE_REGISTRY_SET.call_once(|| unsafe { init_registry(Configuration::new()).unwrap() });
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

impl Registry {
    pub fn new(mut configuration: Configuration) -> Result<Arc<Registry>, Box<Error>> {
        let n_threads = configuration.get_num_threads();

        let (inj_worker, inj_stealer) = deque::new();
        let (workers, stealers): (Vec<_>, Vec<_>) = (0..n_threads).map(|_| deque::new()).unzip();

        let registry = Arc::new(Registry {
            thread_infos: stealers.into_iter()
                .map(|s| ThreadInfo::new(s))
                .collect(),
            state: Mutex::new(RegistryState::new(inj_worker)),
            sleep: Sleep::new(),
            job_uninjector: inj_stealer,
            terminate_latch: CountLatch::new(),
            panic_handler: configuration.take_panic_handler(),
            start_handler: configuration.take_start_handler(),
            exit_handler: configuration.take_exit_handler(),
        });

        // If we return early or panic, make sure to terminate existing threads.
        let t1000 = Terminator(&registry);

        for (index, worker) in workers.into_iter().enumerate() {
            let registry = registry.clone();
            let mut b = thread::Builder::new();
            if let Some(name) = configuration.get_thread_name(index) {
                b = b.name(name);
            }
            if let Some(stack_size) = configuration.get_stack_size() {
                b = b.stack_size(stack_size);
            }
            try!(b.spawn(move || unsafe { main_loop(worker, registry, index) }));
        }

        // Returning normally now, without termination.
        mem::forget(t1000);

        Ok(registry.clone())
    }

    pub fn current() -> Arc<Registry> {
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
    pub fn current_num_threads() -> usize {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                global_registry().num_threads()
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
    #[cfg(test)]
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
        unsafe {
            let worker_thread = WorkerThread::current();
            if !worker_thread.is_null() && (*worker_thread).registry().id() == self.id() {
                (*worker_thread).push(job_ref);
            } else {
                self.inject(&[job_ref]);
            }
        }
    }

    /// Unsafe: caller asserts that injected jobs will remain valid
    /// until they are executed.
    pub unsafe fn inject(&self, injected_jobs: &[JobRef]) {
        log!(InjectJobs { count: injected_jobs.len() });
        {
            let state = self.state.lock().unwrap();

            // It should not be possible for `state.terminate` to be true
            // here. It is only set to true when the user creates (and
            // drops) a `ThreadPool`; and, in that case, they cannot be
            // calling `inject()` later, since they dropped their
            // `ThreadPool`.
            assert!(!self.terminate_latch.probe(), "inject() sees state.terminate as true");

            for &job_ref in injected_jobs {
                state.job_injector.push(job_ref);
            }
        }
        self.sleep.tickle(usize::MAX);
    }

    fn pop_injected_job(&self, worker_index: usize) -> Option<JobRef> {
        loop {
            match self.job_uninjector.steal() {
                Stolen::Empty => return None,
                Stolen::Abort => (), // retry
                Stolen::Data(v) => {
                    log!(UninjectedWork { worker: worker_index });
                    return Some(v);
                }
            }
        }
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
    /// The exception is `spawn_async()`, which can create a job
    /// outside of any blocking scope. In that case, the job itself
    /// holds a terminate count and is responsible for invoking
    /// `terminate()` when finished.
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegistryId {
    addr: usize
}

impl RegistryState {
    pub fn new(job_injector: Worker<JobRef>) -> RegistryState {
        RegistryState {
            job_injector: job_injector,
        }
    }
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
            stealer: stealer,
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// WorkerThread identifiers

pub struct WorkerThread {
    worker: Worker<JobRef>,
    index: usize,

    /// A weak random number generator.
    rng: UnsafeCell<rand::XorShiftRng>,

    registry: Arc<Registry>,
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

    /// Pop `job` from top of stack, returning `false` if it has been
    /// stolen.
    #[inline]
    pub unsafe fn pop(&self) -> Option<JobRef> {
        self.worker.pop()
    }

    /// Wait until the latch is set. Try to keep busy by popping and
    /// stealing tasks as necessary.
    #[inline]
    pub unsafe fn wait_until<L: LatchProbe + ?Sized>(&self, latch: &L) {
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
            if let Some(job) = self.pop()
                                   .or_else(|| self.steal())
                                   .or_else(|| self.registry.pop_injected_job(self.index)) {
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

    pub unsafe fn execute(&self, job: JobRef) {
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
                loop {
                    match victim.stealer.steal() {
                        Stolen::Empty => return None,
                        Stolen::Abort => (), // retry
                        Stolen::Data(v) => {
                            log!(StoleWork { worker: self.index, victim: victim_index });
                            return Some(v);
                        }
                    }
                }
            })
            .next()
    }
}

/// ////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(worker: Worker<JobRef>, registry: Arc<Registry>, index: usize) {
    let worker_thread = WorkerThread {
        worker: worker,
        index: index,
        rng: UnsafeCell::new(rand::weak_rng()),
        registry: registry.clone(),
    };
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

    worker_thread.wait_until(&registry.terminate_latch);

    // Should not be any work left in our queue.
    debug_assert!(worker_thread.pop().is_none());

    // let registry know we are done
    registry.thread_infos[index].stopped.set();

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
/// panic will be propagated as well.
pub fn in_worker<OP, R>(op: OP) -> R
    where OP: FnOnce(&WorkerThread) -> R + Send, R: Send
{
    unsafe {
        let owner_thread = WorkerThread::current();
        if !owner_thread.is_null() {
            // Perfectly valid to give them a `&T`: this is the
            // current thread, so we know the data structure won't be
            // invalidated until we return.
            return op(&*owner_thread);
        } else {
            return in_worker_cold(op);
        }
    }
}

#[cold]
unsafe fn in_worker_cold<OP, R>(op: OP) -> R
    where OP: FnOnce(&WorkerThread) -> R + Send, R: Send
{
    // never run from a worker thread; just shifts over into worker threads
    debug_assert!(WorkerThread::current().is_null());
    let job = StackJob::new(|| in_worker(op), LockLatch::new());
    global_registry().inject(&[job.as_job_ref()]);
    job.latch.wait();
    job.into_result()
}
