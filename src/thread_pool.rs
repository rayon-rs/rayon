use Configuration;
use deque;
use deque::{Worker, Stealer, Stolen};
use job::JobRef;
use latch::{Latch, LockLatch, SpinLatch};
#[allow(unused_imports)]
use log::Event::*;
use rand;
use std::cell::Cell;
use std::sync::{Arc, Condvar, Mutex, Once, ONCE_INIT};
use std::thread;
use std::collections::VecDeque;
use std::mem;
use util::leak;
use num_cpus;

///////////////////////////////////////////////////////////////////////////

pub struct Registry {
    thread_infos: Vec<ThreadInfo>,
    state: Mutex<RegistryState>,
    work_available: Condvar,
}

struct RegistryState {
    terminate: bool,
    threads_at_work: usize,
    injected_jobs: VecDeque<JobRef>,
}

///////////////////////////////////////////////////////////////////////////
// Initialization

static mut THE_REGISTRY: Option<&'static Registry> = None;
static THE_REGISTRY_SET: Once = ONCE_INIT;

/// Starts the worker threads (if that has not already happened). If
/// initialization has not already occurred, use the default
/// configuration.
pub fn get_registry() -> &'static Registry {
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
    let registry = leak(Registry::new(config.num_threads()));
    THE_REGISTRY = Some(registry);
}

enum Work {
    None,
    Job(JobRef),
    Terminate,
}

impl Registry {
    pub fn new(num_threads: Option<usize>) -> Arc<Registry> {
        let limit_value = match num_threads {
            Some(value) => value,
            None => num_cpus::get()
        };

        let (workers, stealers) : (Vec<_>, Vec<_>) =
            (0..limit_value).map(|_| deque::new()).unzip();

        let registry = Arc::new(Registry {
            thread_infos: stealers.into_iter().map(|s| ThreadInfo::new(s))
                                          .collect(),
            state: Mutex::new(RegistryState::new()),
            work_available: Condvar::new(),
        });

        for (index, worker) in workers.into_iter().enumerate() {
            let registry = registry.clone();
            thread::spawn(move || unsafe { main_loop(worker, registry, index) });
        }

        registry
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

    pub unsafe fn inject(&self, injected_jobs: &[JobRef]) {
        log!(InjectJobs { count: injected_jobs.len() });
        let mut state = self.state.lock().unwrap();
        if state.terminate {
            drop(state);
            // We can only reach this point if these conditions are met:
            // 1) This must be the global thread pool
            // 2) The only way to terminate the global thread pool is if a
            //    worker thread panics.
            // 3) A worker thread can only panic if a job panicked and was not
            //    caught by panic::recover.
            panic!("rayon thread pool is contaminated by a previous panic; recovery is only available on nightly compilers");
        }
        state.injected_jobs.extend(injected_jobs);
        self.work_available.notify_all();
    }

    fn wait_for_work(&self, _worker: usize, was_active: bool) -> Work {
        log!(WaitForWork { worker: _worker, was_active: was_active });

        let mut state = self.state.lock().unwrap();

        if was_active {
            state.threads_at_work -= 1;
        }

        loop {
            // Check if we need to terminate.
            if state.terminate {
                return Work::Terminate;
            }

            // Otherwise, if anything was injected from outside,
            // return that.  Note that this gives preference to
            // injected items over stealing from others, which is a
            // bit dubious, but then so is the opposite.
            if let Some(job) = state.injected_jobs.pop_front() {
                state.threads_at_work += 1;
                self.work_available.notify_all();
                return Work::Job(job);
            }

            // If any of the threads are running a job, we should spin
            // up, since they may generate subworkitems.
            if state.threads_at_work > 0 {
                return Work::None;
            }

            state = self.work_available.wait(state).unwrap();
        }
    }

    pub fn terminate(&self) {
        let mut state = self.state.lock().unwrap();
        state.terminate = true;
        for job in state.injected_jobs.drain(..) {
            unsafe {
                (*job.0).abort();
            }
        }
        self.work_available.notify_all();
    }
}

impl RegistryState {
    pub fn new() -> RegistryState {
        RegistryState {
            threads_at_work: 0,
            injected_jobs: VecDeque::new(),
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

///////////////////////////////////////////////////////////////////////////
// WorkerThread identifiers

pub struct WorkerThread {
    worker: Worker<JobRef>,
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
    pub unsafe fn push(&self, job: JobRef) {
        self.worker.push(job);
    }

    /// Pop `job` from top of stack, returning `false` if it has been
    /// stolen.
    #[inline]
    pub unsafe fn pop(&self) -> bool {
        self.worker.pop().is_some()
    }

    #[cold]
    pub unsafe fn steal_until(&self, latch: &SpinLatch) {
        // If another thread stole our job when we panic, we must halt unwinding
        // until that thread is finished using it.
        struct PanicGuard<'a>(&'a SpinLatch);
        impl<'a> Drop for PanicGuard<'a> {
            fn drop(&mut self) {
                while !self.0.probe() {
                    thread::yield_now();
                }
            }
        }

        let guard = PanicGuard(&latch);
        while !latch.probe() {
            if let Some(job) = steal_work(&self.registry, self.index) {
                (*job.0).execute();
            } else {
                thread::yield_now();
            }
        }
        mem::forget(guard);
    }
}

///////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(worker: Worker<JobRef>, registry: Arc<Registry>, index: usize) {
    let worker_thread = WorkerThread {
        worker: worker,
        registry: registry.clone(),
        index: index,
    };
    worker_thread.set_current();

    // let registry know we are ready to do work
    registry.thread_infos[index].primed.set();

    // If a worker thread panics, bring down the entire thread pool
    struct PanicGuard;
    impl Drop for PanicGuard {
        fn drop(&mut self) {
            unsafe {
                (*WorkerThread::current()).registry.terminate();
            }
        }
    }
    let _guard = PanicGuard;

    let mut was_active = false;
    loop {
        match registry.wait_for_work(index, was_active) {
            Work::Job(injected_job) => {
                (*injected_job.0).execute();
                was_active = true;
                continue;
            }
            Work::Terminate => break,
            Work::None => {},
        }

        if let Some(stolen_job) = steal_work(&registry, index) {
            log!(StoleWork { worker: index });
            registry.start_working(index);
            (*stolen_job.0).execute();
            was_active = true;
        } else {
            was_active = false;
        }
    }
}

unsafe fn steal_work(registry: &Registry, index: usize) -> Option<JobRef> {
    let num_threads = registry.num_threads();
    let start = rand::random::<usize>() % num_threads;
    (start .. num_threads)
        .chain(0 .. start)
        .filter(|&i| i != index)
        .flat_map(|i| match registry.thread_infos[i].stealer.steal() {
            Stolen::Empty => None,
            Stolen::Abort => None, // loop?
            Stolen::Data(v) => Some(v),
        })
        .next()
}
