use latch::{LockLatch, SpinLatch};
#[allow(unused_imports)]
use log::Event::*;
use job::{Code, CodeImpl, Job};
use std::sync::Arc;
use thread_pool::{self, Registry, WorkerThread};

/// Custom error type for the rayon thread pool configuration.
#[derive(Debug,PartialEq)]
pub enum InitResult {
    /// Error if number of threads is set to zero.
    NumberOfThreadsZero,
    /// Error if thread pool is initialized multiple times and the number of threads
    /// is not equal for all configurations.
    NumberOfThreadsNotEqual
}

/// Contains the rayon thread pool configuration.
#[derive(Clone, Debug)]
pub struct Configuration {
    /// The number of threads in the rayon thread pool. Must not be zero.
    num_threads: Option<usize>
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
    /// The argument `num_threads` must not be zero. If you do not call this function,
    /// rayon will select a suitable default (currently, the default is one thread per
    /// CPU respectively core).
    pub fn set_num_threads(mut self, num_threads: usize) -> Configuration {
        self.num_threads = Some(num_threads);
        self
    }

    /// Initializes the global thread pool. Returns `Ok(())` if the configuration is valid.
    /// The initialisation happens exactly once. Every subsequent call to initialize has
    /// no effect.
    /// If the number of threads is zero `Err(NumberOfThreadsZero)` is returned.
    /// If the configuration is initialized more than one time and the number of threads
    /// is not equall for every configuration then `Err(NumberOfThreadsNotEqual)` is returned.
    pub fn initialize(self) -> Result<(), InitResult> {
        let num_threads = self.num_threads;

        if let Some(value) = num_threads {
            if value == 0 {
                return Err(InitResult::NumberOfThreadsZero);
            }
        }

        let registry = thread_pool::get_registry_with_config(self);

        if let Some(value) = num_threads {
            if value != registry.num_threads() {
                return Err(InitResult::NumberOfThreadsNotEqual);
            }
        }

        registry.wait_until_primed();

        Ok(())
    }
}

/// This is a debugging API not really intended for end users. It will
/// dump some performance statistics out using `println`.
pub fn dump_stats() {
    dump_stats!();
}

pub fn join<A,B,RA,RB>(oper_a: A,
                       oper_b: B)
                       -> (RA, RB)
    where A: FnOnce() -> RA + Send,
          B: FnOnce() -> RB + Send,
          RA: Send,
          RB: Send,
{
    unsafe {
        let worker_thread = WorkerThread::current();

        // slow path: not yet in the thread pool
        if worker_thread.is_null() {
            return join_inject(oper_a, oper_b);
        }

        log!(Join { worker: (*worker_thread).index() });

        // create a home where we will write result of task b
        let mut result_b = None;

        // create virtual wrapper for task b; this all has to be
        // done here so that the stack frame can keep it all live
        // long enough
        let mut code_b = CodeImpl::new(oper_b, &mut result_b);
        let mut latch_b = SpinLatch::new();
        let mut job_b = Job::new(&mut code_b, &mut latch_b);
        (*worker_thread).push(&mut job_b);

        // execute task a; hopefully b gets stolen
        let result_a = oper_a();

        // if b was not stolen, do it ourselves, else wait for the thief to finish
        if (*worker_thread).pop() {
            log!(PoppedJob { worker: (*worker_thread).index() });
            code_b.execute(); // not stolen, let's do it!
        } else {
            log!(LostJob { worker: (*worker_thread).index() });
            (*worker_thread).steal_until(&latch_b); // stolen, wait for them to finish
        }

        // now result_b should be initialized
        (result_a, result_b.unwrap())
    }
}

#[inline(never)] // cold path
unsafe fn join_inject<A,B,RA,RB>(oper_a: A,
                                 oper_b: B)
                                 -> (RA, RB)
    where A: FnOnce() -> RA + Send,
          B: FnOnce() -> RB + Send,
          RA: Send,
          RB: Send,
{
    let mut result_a = None;
    let mut code_a = CodeImpl::new(oper_a, &mut result_a);
    let mut latch_a = LockLatch::new();
    let mut job_a = Job::new(&mut code_a, &mut latch_a);

    let mut result_b = None;
    let mut code_b = CodeImpl::new(oper_b, &mut result_b);
    let mut latch_b = LockLatch::new();
    let mut job_b = Job::new(&mut code_b, &mut latch_b);

    thread_pool::get_registry().inject(&[&mut job_a, &mut job_b]);

    latch_a.wait();
    latch_b.wait();

    (result_a.unwrap(), result_b.unwrap())
}

pub struct ThreadPool {
    registry: Arc<Registry>
}

impl ThreadPool {
    /// Constructs a new thread pool.
    /// Expects a valid configuration: if the number of threads is not explicitly set via `set_num_threads()`,
    /// the current number of cores (CPUs) is used as determined via `num_cpus::get()`.
    /// This function returns `Error(NumberOfThreadsZero)` if the number of threads is zero
    /// otherwise it returns `Ok(Threadpool)`.
    pub fn new(configuration: Configuration) -> Result<ThreadPool,InitResult> {
        if let Some(value) = configuration.num_threads {
            if value == 0 {
                return Err(InitResult::NumberOfThreadsZero);
            }
        }
        Ok(ThreadPool {
            registry: Registry::new(configuration.num_threads)
        })
    }

    /// Executes `op` within the threadpool. Any attempts to `join`
    /// which occur there will then operate within that threadpool.
    pub fn install<OP,R>(&self, op: OP) -> R
        where OP: FnOnce() -> R + Send
    {
        unsafe {
            let mut result_a = None;
            let mut code_a = CodeImpl::new(op, &mut result_a);
            let mut latch_a = LockLatch::new();
            let mut job_a = Job::new(&mut code_a, &mut latch_a);
            self.registry.inject(&[&mut job_a]);
            latch_a.wait();
            result_a.unwrap()
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.registry.terminate();
    }
}
