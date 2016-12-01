use Configuration;
use deque;
use deque::{Worker, Stealer, Stolen};
use job::{JobRef, JobMode, StackJob};
use latch::{Latch, LockLatch, SpinLatch};
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
}

struct RegistryState {
    terminate: bool,
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
    let registry = leak(Arc::new(Registry::new(config.num_threads())));
    THE_REGISTRY = Some(registry);
}

impl Registry {
    pub fn new(num_threads: Option<usize>) -> Arc<Registry> {
        let limit_value = match num_threads {
            Some(value) => value,
            None => match env::var("RAYON_RS_NUM_CPUS") {
                Ok(s) => usize::from_str(&s).expect("invalid value for RAYON_RS_NUM_CPUS"),
                Err(_) => num_cpus::get(),
            },
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
        });

        for (index, worker) in workers.into_iter().enumerate() {
            let registry = registry.clone();
            thread::spawn(move || unsafe { main_loop(worker, registry, index) });
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
            assert!(!state.terminate, "inject() sees state.terminate as true");

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

    pub fn terminate(&self) {
        {
            let mut state = self.state.lock().unwrap();
            state.terminate = true;
            while let Some(job) = state.job_injector.pop() {
                unsafe {
                    job.execute(JobMode::Abort);
                }
            }
        }
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
            terminate: false,
        }
    }
}

struct ThreadInfo {
    // latch is set once thread has started and we are entering into
    // the main loop
    primed: LockLatch,
    stealer: Stealer<JobRef>,
}

impl ThreadInfo {
    fn new(stealer: Stealer<JobRef>) -> ThreadInfo {
        ThreadInfo {
            primed: LockLatch::new(),
            stealer: stealer,
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// WorkerThread identifiers

pub struct WorkerThread {
    worker: Worker<JobRef>,
    stealers: Vec<(usize, Stealer<JobRef>)>,
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
        job.execute(JobMode::Execute);

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
        if self.stealers.is_empty() {
            return None;
        }

        let start = {
            // OK to use this UnsafeCell because (a) this data is
            // confined to current thread, as WorkerThread is not Send
            // nor Sync and (b) rand crate will not call back into
            // this method.
            let rng = &mut *self.rng.get();
            rng.next_u32() % self.stealers.len() as u32
        };
        let (lo, hi) = self.stealers.split_at(start as usize);
        hi.iter()
            .chain(lo)
            .filter_map(|&(victim_index, ref stealer)| {
                loop {
                    match stealer.steal() {
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
    let stealers = registry.thread_infos
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != index)
        .map(|(i, ti)| (i, ti.stealer.clone()))
        .collect::<Vec<_>>();

    assert!(stealers.len() < ::std::u32::MAX as usize,
            "We assume this is not going to happen!");

    let worker_thread = WorkerThread {
        worker: worker,
        stealers: stealers,
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

    let dummy_latch = SpinLatch::new();
    worker_thread.wait_until(&dummy_latch);

    // Normal termination, do not abort.
    mem::forget(abort_guard);
}

pub fn in_worker<OP>(op: OP)
    where OP: FnOnce(&WorkerThread) + Send
{
    unsafe {
        let owner_thread = WorkerThread::current();
        if !owner_thread.is_null() {
            // Perfectly valid to give them a `&T`: this is the
            // current thread, so we know the data structure won't be
            // invalidated until we return.
            op(&*owner_thread);
        } else {
            in_worker_cold(op);
        }
    }
}

#[cold]
unsafe fn in_worker_cold<OP>(op: OP)
    where OP: FnOnce(&WorkerThread) + Send
{
    // never run from a worker thread; just shifts over into worker threads
    debug_assert!(WorkerThread::current().is_null());
    let job = StackJob::new(|| in_worker(op), LockLatch::new());
    global_registry().inject(&[job.as_job_ref()]);
    job.latch.wait();
}
