use Configuration;
use deque;
use deque::{Worker, Stealer, Stolen};
use job::{JobRef, StackJob};
use latch::{Latch, CountLatch, LockLatch};
#[allow(unused_imports)]
use log::Event::*;
use rand::{self, Rng};
use sleep::Sleep;
use std::cell::{Cell, UnsafeCell};
use std::env;
use std::str::FromStr;
use std::sync::{Arc, Mutex, Once, ONCE_INIT};
use std::thread;
use std::mem;
use std::u32;
use std::usize;
use unwind;
use util::leak;
use num_cpus;

/// ////////////////////////////////////////////////////////////////////////

pub struct Registry {
    thread_infos: Vec<ThreadInfo>,
    state: Mutex<RegistryState>,
    sleep: Sleep,
    job_uninjector: Stealer<JobRef>,

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
pub fn global_registry() -> &'static Arc<Registry> {
    THE_REGISTRY_SET.call_once(|| unsafe { init_registry(Configuration::new()) });
    unsafe { THE_REGISTRY.unwrap() }
}

/// Starts the worker threads (if that has not already happened) with
/// the given configuration.
pub fn get_registry_with_config(config: Configuration) -> &'static Registry {
    THE_REGISTRY_SET.call_once(|| unsafe { init_registry(config) });
    unsafe { THE_REGISTRY.unwrap() }
}

/// Initializes the global registry with the given configuration.
/// Meant to be called from within the `THE_REGISTRY_SET` once
/// function. Declared `unsafe` because it writes to `THE_REGISTRY` in
/// an unsynchronized fashion.
unsafe fn init_registry(config: Configuration) {
    let registry = leak(Arc::new(Registry::new(config)));
    THE_REGISTRY = Some(registry);
}

impl Registry {
    pub fn new(mut configuration: Configuration) -> Arc<Registry> {
        let limit_value = match configuration.num_threads() {
            Some(value) => value,
            None => {
                match env::var("RAYON_RS_NUM_CPUS") {
                    Ok(s) => usize::from_str(&s).expect("invalid value for RAYON_RS_NUM_CPUS"),
                    Err(_) => num_cpus::get(),
                }
            }
        };

        let (inj_worker, inj_stealer) = deque::new();
        let (workers, stealers): (Vec<_>, Vec<_>) = (0..limit_value).map(|_| deque::new()).unzip();

        let registry = Arc::new(Registry {
            thread_infos: stealers.into_iter()
                .map(|s| ThreadInfo::new(s))
                .collect(),
            state: Mutex::new(RegistryState::new(inj_worker)),
            sleep: Sleep::new(),
            job_uninjector: inj_stealer,
            terminate_latch: CountLatch::new(),
        });

        for (index, worker) in workers.into_iter().enumerate() {
            let registry = registry.clone();
            let mut b = thread::Builder::new();
            if let Some(name) = configuration.thread_name(index) {
                b = b.name(name);
            }
            // FIXME(#205) recover from this error
            b.spawn(move || unsafe { main_loop(worker, registry, index) }).unwrap();
        }

        registry
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

    pub unsafe fn inject(&self, injected_jobs: &[JobRef]) {
        log!(InjectJobs { count: injected_jobs.len() });
        {
            let state = self.state.lock().unwrap();

            // It should not be possible for `state.terminate` to be true
            // here. It is only set to true when the user creates (and
            // drops) a `ThreadPool`; and, in that case, they cannot be
            // calling `inject()` later, since they dropped their
            // `ThreadPool`.
            assert!(!self.terminate_latch.probe(),
                    "inject() sees state.terminate as true");

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
    addr: usize,
}

impl RegistryState {
    pub fn new(job_injector: Worker<JobRef>) -> RegistryState {
        RegistryState { job_injector: job_injector }
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
    pub unsafe fn current() -> *const WorkerThread {
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
    pub unsafe fn wait_until<L: Latch>(&self, latch: &L) {
        log!(WaitUntil { worker: self.index });
        if !latch.probe() {
            self.wait_until_cold(latch);
        }
    }

    #[cold]
    unsafe fn wait_until_cold<L: Latch>(&self, latch: &L) {
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
        (start..num_threads)
            .chain(0..start)
            .filter(|&i| i != self.index)
            .filter_map(|victim_index| {
                let victim = &self.registry.thread_infos[victim_index];
                loop {
                    match victim.stealer.steal() {
                        Stolen::Empty => return None,
                        Stolen::Abort => (), // retry
                        Stolen::Data(v) => {
                            log!(StoleWork {
                                worker: self.index,
                                victim: victim_index,
                            });
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

    worker_thread.wait_until(&registry.terminate_latch);

    // Should not be any work left in our queue.
    debug_assert!(worker_thread.pop().is_none());

    // let registry know we are done
    registry.thread_infos[index].stopped.set();

    // Normal termination, do not abort.
    mem::forget(abort_guard);
}

/// If already in a worker-thread, just execute `op`.  Otherwise,
/// execute `op` in the default thread-pool. Either way, block until
/// `op` completes and return its return value. If `op` panics, that
/// panic will be propagated as well.
pub fn in_worker<OP, R>(op: OP) -> R
    where OP: FnOnce(&WorkerThread) -> R + Send,
          R: Send
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
    where OP: FnOnce(&WorkerThread) -> R + Send,
          R: Send
{
    // never run from a worker thread; just shifts over into worker threads
    debug_assert!(WorkerThread::current().is_null());
    let job = StackJob::new(|| in_worker(op), LockLatch::new());
    global_registry().inject(&[job.as_job_ref()]);
    job.latch.wait();
    job.into_result()
}
