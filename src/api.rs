use latch::{LockLatch, SpinLatch};
#[allow(unused_imports)]
use log::Event::*;
use job::StackJob;
use std::cell::{Ref, RefCell, RefMut};
use std::sync::Arc;
use std::error::Error;
use std::fmt;
use thread_pool::{self, Registry, WorkerThread};
use std::mem;
use unwind;

/// Custom error type for the rayon thread pool configuration.
#[derive(Debug,PartialEq)]
pub enum InitError {
    /// Error if number of threads is set to zero.
    NumberOfThreadsZero,

    /// Error if the gloal thread pool is initialized multiple times
    /// and the configuration is not equal for all configurations.
    GlobalPoolAlreadyInitialized,
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            InitError::NumberOfThreadsZero => {
                write!(f,
                       "The number of threads was set to zero but must be greater than zero.")
            }
            InitError::GlobalPoolAlreadyInitialized => {
                write!(f,
                       "The gobal thread pool has already been initialized with a different \
                        configuration. Only one valid configuration is allowed.")
            }
        }
    }
}

impl Error for InitError {
    fn description(&self) -> &str {
        match *self {
            InitError::NumberOfThreadsZero => "number of threads set to zero",
            InitError::GlobalPoolAlreadyInitialized => {
                "global thread pool has already been initialized"
            }
        }
    }
}

/// Contains the rayon thread pool configuration.
#[derive(Clone, Debug)]
pub struct Configuration {
    /// The number of threads in the rayon thread pool. Must not be zero.
    num_threads: Option<usize>,
}

impl Configuration {
    /// Creates and return a valid rayon thread pool configuration, but does not initialize it.
    pub fn new() -> Configuration {
        Configuration { num_threads: None }
    }

    /// Get the number of threads that will be used for the thread
    /// pool. See `set_num_threads` for more information.
    pub fn num_threads(&self) -> Option<usize> {
        self.num_threads
    }

    /// Set the number of threads to be used in the rayon threadpool.
    /// The argument `num_threads` must not be zero. If you do not
    /// call this function, rayon will select a suitable default
    /// (currently, the default is one thread per CPU core).
    pub fn set_num_threads(mut self, num_threads: usize) -> Configuration {
        self.num_threads = Some(num_threads);
        self
    }

    /// Checks whether the configuration is valid.
    fn validate(&self) -> Result<(), InitError> {
        if let Some(value) = self.num_threads {
            if value == 0 {
                return Err(InitError::NumberOfThreadsZero);
            }
        }

        Ok(())
    }
}

/// Initializes the global thread pool. This initialization is
/// **optional**.  If you do not call this function, the thread pool
/// will be automatically initialized with the default
/// configuration. In fact, calling `initialize` is not recommended,
/// except for in two scenarios:
///
/// - You wish to change the default configuration.
/// - You are running a benchmark, in which case initializing may
///   yield slightly more consistent results, since the worker threads
///   will already be ready to go even in the first iteration.  But
///   this cost is minimal.
///
/// Initialization of the global thread pool happens exactly
/// once. Once started, the configuration cannot be
/// changed. Therefore, if you call `initialize` a second time, it
/// will simply check that the global thread pool already has the
/// configuration you requested, rather than making changes.
///
/// An `Ok` result indicates that the thread pool is running with the
/// given configuration. Otherwise, a suitable error is returned.
pub fn initialize(config: Configuration) -> Result<(), InitError> {
    try!(config.validate());

    let num_threads = config.num_threads;

    let registry = thread_pool::get_registry_with_config(config);

    if let Some(value) = num_threads {
        if value != registry.num_threads() {
            return Err(InitError::GlobalPoolAlreadyInitialized);
        }
    }

    registry.wait_until_primed();

    Ok(())
}

/// This is a debugging API not really intended for end users. It will
/// dump some performance statistics out using `println`.
pub fn dump_stats() {
    dump_stats!();
}

/// The `join` function takes two closures and potentially runs them in parallel but is not
/// guaranteed to. However, the call to `join` incurs low overhead and is much different compared
/// to spawning two separate threads.
pub fn join<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
    where A: FnOnce() -> RA + Send,
          B: FnOnce() -> RB + Send,
          RA: Send,
          RB: Send
{
    unsafe {
        let worker_thread = WorkerThread::current();

        // slow path: not yet in the thread pool
        if worker_thread.is_null() {
            return join_inject(oper_a, oper_b);
        }

        log!(Join { worker: (*worker_thread).index() });

        // create virtual wrapper for task b; this all has to be
        // done here so that the stack frame can keep it all live
        // long enough
        let job_b = StackJob::new(oper_b, SpinLatch::new());
        (*worker_thread).push(job_b.as_job_ref());

        // record how many async spawns have occurred on this thread
        // before task A is executed
        let spawn_count = (*worker_thread).current_spawn_count();

        // execute task a; hopefully b gets stolen
        let result_a;
        {
            let guard = unwind::finally(&job_b.latch, |job_b_latch| {
                // If another thread stole our job when we panic, we must halt unwinding
                // until that thread is finished using it.
                if (*WorkerThread::current()).pop().is_none() {
                    job_b_latch.spin();
                }
            });
            result_a = oper_a();
            mem::forget(guard);
        }

        // before we can try to pop b, we have to first pop off any async spawns
        // that have occurred on this thread
        (*worker_thread).pop_spawned_jobs(spawn_count);

        // if b was not stolen, do it ourselves, else wait for the thief to finish
        let result_b;
        if (*worker_thread).pop().is_some() {
            log!(PoppedJob { worker: (*worker_thread).index() });
            result_b = job_b.run_inline(); // not stolen, let's do it!
        } else {
            log!(LostJob { worker: (*worker_thread).index() });
            (*worker_thread).steal_until(&job_b.latch); // stolen, wait for them to finish
            result_b = job_b.into_result();
        }

        // now result_b should be initialized
        (result_a, result_b)
    }
}

#[cold] // cold path
unsafe fn join_inject<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
    where A: FnOnce() -> RA + Send,
          B: FnOnce() -> RB + Send,
          RA: Send,
          RB: Send
{
    let job_a = StackJob::new(oper_a, LockLatch::new());
    let job_b = StackJob::new(oper_b, LockLatch::new());

    thread_pool::get_registry().inject(&[job_a.as_job_ref(), job_b.as_job_ref()]);

    job_a.latch.wait();
    job_b.latch.wait();

    (job_a.into_result(), job_b.into_result())
}

pub struct ThreadPool {
    registry: Arc<Registry>,
}

impl ThreadPool {
    /// Constructs a new thread pool with the given configuration. If
    /// the configuration is not valid, returns a suitable `Err`
    /// result.  See `InitError` for more details.
    pub fn new(configuration: Configuration) -> Result<ThreadPool, InitError> {
        try!(configuration.validate());
        Ok(ThreadPool { registry: Registry::new(configuration.num_threads) })
    }

    /// Executes `op` within the threadpool. Any attempts to `join`
    /// which occur there will then operate within that threadpool.
    pub fn install<OP, R>(&self, op: OP) -> R
        where OP: FnOnce() -> R + Send
    {
        unsafe {
            let job_a = StackJob::new(op, LockLatch::new());
            self.registry.inject(&[job_a.as_job_ref()]);
            job_a.latch.wait();
            job_a.into_result()
        }
    }

    /// Returns the number of threads in the thread pool.
    pub fn num_threads(&self) -> usize {
        self.registry.num_threads()
    }

    /// If called from a Rayon worker thread in this thread-pool,
    /// returns the index of that thread; if not called from a Rayon
    /// thread, or called from a Rayon thread that belongs to a
    /// different thread-pool, returns `None`.
    ///
    /// The index for a given thread will not change over the thread's
    /// lifetime. However, multiple threads may share the same index if
    /// they are in distinct thread-pools.
    ///
    /// ### Future compatibility note
    ///
    /// Currently, every thread-pool (including the global thread-pool)
    /// has a fixed number of threads, but this may change in future Rayon
    /// versions. In that case, the index for a thread would not change
    /// during its lifetime, but thread indices may wind up being reused
    /// if threads are terminated and restarted. (If this winds up being
    /// an untenable policy, then a semver-incompatible version can be
    /// used.)
    pub fn current_thread_index(&self) -> Option<usize> {
        unsafe {
            let curr = WorkerThread::current();
            if curr.is_null() {
                None
            } else if (*curr).registry().id() != self.registry.id() {
                None
            } else {
                Some((*curr).index())
            }
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.registry.terminate();
    }
}

/// Stack-scoped thread-local storage for thread pools.
pub struct ScopedTLS<'a, T: Send> {
    _pool: &'a ThreadPool,
    slots: Box<[RefCell<Option<T>>]>,
}

unsafe impl<'a, T: Send> Sync for ScopedTLS<'a, T> {}

impl<'a, T: Send> ScopedTLS<'a, T> {
    pub fn new(p: &'a ThreadPool) -> Self {
        let mut v = Vec::new();
        for _ in 0..p.registry.num_threads() {
            v.push(RefCell::new(None));
        }

        ScopedTLS {
            _pool: p,
            slots: v.into_boxed_slice(),
        }
    }

    pub fn borrow(&self) -> Ref<Option<T>> {
        let idx = current_thread_index();
        self.slots[idx].borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<Option<T>> {
        let idx = current_thread_index();
        self.slots[idx].borrow_mut()
    }
}

fn current_thread_index() -> usize {
    unsafe {
        let curr = WorkerThread::current();
        assert!(!curr.is_null());
        (*curr).index()
    }
}
