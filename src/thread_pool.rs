use deque::{BufferPool, Worker, Stealer, Stolen};
use job::Job;
use latch::{Latch, LockLatch, SpinLatch};
#[allow(unused_imports)]
use log::Event::*;
use num_cpus;
use rand;
use std::cell::Cell;
use std::sync::{Arc, Condvar, Mutex, Once, ONCE_INIT};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use util::leak;

///////////////////////////////////////////////////////////////////////////

pub struct Registry {
    thread_infos: Vec<ThreadInfo>,
    state: Mutex<RegistryState>,
    work_available: Condvar,
    terminate: AtomicBool,
}

struct RegistryState {
    threads_at_work: usize,
    injected_jobs: Vec<*mut Job<LockLatch>>,
}

unsafe impl Send for Registry { }
unsafe impl Sync for Registry { }

///////////////////////////////////////////////////////////////////////////
// Initialization

static mut THE_REGISTRY: Option<&'static Registry> = None;
static THE_REGISTRY_SET: Once = ONCE_INIT;

/// Starts the worker threads (if that has not already happened) and
/// returns the registry.
pub fn get_registry() -> &'static Registry {
    THE_REGISTRY_SET.call_once(|| {
        let registry = leak(Registry::new());
        unsafe { THE_REGISTRY = Some(registry); }
    });
    unsafe { THE_REGISTRY.unwrap() }
}

impl Registry {
    pub fn new() -> Arc<Registry> {
        let num_threads = num_cpus::get();
        let pool = BufferPool::new();
        let registry = Arc::new(Registry {
            thread_infos: (0..num_threads).map(|_| ThreadInfo::new(&pool))
                                          .collect(),
            state: Mutex::new(RegistryState::new()),
            work_available: Condvar::new(),
            terminate: AtomicBool::new(false),
        });

        for index in 0 .. num_threads {
            let registry = registry.clone();
            thread::spawn(move || unsafe { main_loop(registry, index) });
        }

        registry
    }

    fn num_threads(&self) -> usize {
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

    ///////////////////////////////////////////////////////////////////////////
    // MAIN LOOP
    //
    // So long as all of the worker threads are hanging out in their
    // top-level loop, there is no work to be done.

    fn start_working(&self, index: usize) {
        log!(StartWorking { index: index });
        let mut state = self.state.lock().unwrap();
        state.threads_at_work += 1;
        self.work_available.notify_all();
    }

    pub unsafe fn inject(&self, injected_jobs: &[*mut Job<LockLatch>]) {
        log!(InjectJobs { count: injected_jobs.len() });
        let mut state = self.state.lock().unwrap();
        state.injected_jobs.extend(injected_jobs);
        self.work_available.notify_all();
    }

    fn wait_for_work(&self, _worker: usize, was_active: bool) -> Option<*mut Job<LockLatch>> {
        log!(WaitForWork { worker: _worker, was_active: was_active });

        let mut state = self.state.lock().unwrap();

        if was_active {
            state.threads_at_work -= 1;
        }

        loop {
            // Otherwise, if anything was injected from outside,
            // return that.  Note that this gives preference to
            // injected items over stealing from others, which is a
            // bit dubious, but then so is the opposite.
            if let Some(job) = state.injected_jobs.pop() {
                state.threads_at_work += 1;
                self.work_available.notify_all();
                return Some(job);
            }

            // If any of the threads are running a job, we should spin
            // up, since they may generate subworkitems.
            if state.threads_at_work > 0 {
                return None;
            }

            state = self.work_available.wait(state).unwrap();
        }
    }

    pub fn terminate(&self) {
        self.terminate.store(true, Ordering::SeqCst);
    }
}

impl RegistryState {
    pub fn new() -> RegistryState {
        RegistryState {
            threads_at_work: 0,
            injected_jobs: Vec::new(),
        }
    }
}

struct ThreadInfo {
    // latch is set once thread has started and we are entering into
    // the main loop
    primed: LockLatch,
    worker: Worker<JobRef>,
    stealer: Stealer<JobRef>,
}

impl ThreadInfo {
    fn new(pool: &BufferPool<JobRef>) -> ThreadInfo {
        let (worker, stealer) = pool.deque();
        ThreadInfo {
            primed: LockLatch::new(),
            worker: worker,
            stealer: stealer,
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// WorkerThread identifiers

pub struct WorkerThread {
    registry: Arc<Registry>,
    index: usize
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
    unsafe fn set_current(&self) {
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().is_null());
            t.set(self);
        });
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    #[inline]
    fn thread_info(&self) -> &ThreadInfo {
        &self.registry.thread_infos[self.index]
    }

    #[inline]
    pub unsafe fn push(&self, job: *mut Job<SpinLatch>) {
        self.thread_info().worker.push(JobRef { ptr: job });
    }

    /// Pop `job` from top of stack, returning `false` if it has been
    /// stolen.
    #[inline]
    pub unsafe fn pop(&self) -> bool {
        self.thread_info().worker.pop().is_some()
    }

    pub unsafe fn steal_until(&self, latch: &SpinLatch) {
        while !latch.probe() {
            if let Some(job) = steal_work(&self.registry, self.index) {
                (*job).execute();
            } else {
                thread::yield_now();
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(registry: Arc<Registry>, index: usize) {
    let worker_thread = WorkerThread {
        registry: registry.clone(),
        index: index,
    };
    worker_thread.set_current();

    // let registry know we are ready to do work
    registry.thread_infos[index].primed.set();

    let mut was_active = false;
    while !registry.terminate.load(Ordering::SeqCst) {
        if let Some(injected_job) = registry.wait_for_work(index, was_active) {
            (*injected_job).execute();
            was_active = true;
        } else if let Some(stolen_job) = steal_work(&registry, index) {
            log!(StoleWork { worker: index, job: stolen_job });
            registry.start_working(index);
            (*stolen_job).execute();
            was_active = true;
        } else {
            was_active = false;
        }
    }
}

unsafe fn steal_work(registry: &Registry, index: usize) -> Option<*mut Job<SpinLatch>> {
    let num_threads = registry.num_threads();
    let start = rand::random::<usize>() % num_threads;
    (start .. num_threads)
        .chain(0 .. start)
        .filter(|&i| i != index)
        .flat_map(|i| match registry.thread_infos[i].stealer.steal() {
            Stolen::Empty => None,
            Stolen::Abort => None, // loop?
            Stolen::Data(v) => Some(v.ptr),
        })
        .next()
}

///////////////////////////////////////////////////////////////////////////

pub struct JobRef {
    pub ptr: *mut Job<SpinLatch>
}

unsafe impl Send for JobRef { }
unsafe impl Sync for JobRef { }

