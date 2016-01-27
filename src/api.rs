use latch::{LockLatch, SpinLatch};
#[allow(unused_imports)]
use log::Event::*;
use job::{Code, CodeImpl, Job};
use std::sync::Arc;
use thread_pool::{self, Registry, WorkerThread};

/// Initializes the Rayon threadpool. You don't normally need to do
/// this, as it happens automatically, but it is handy for
/// benchmarking purposes since it avoids initialization overhead in
/// the actual operations.
/// But you need to call this if you want to specify the maximum number
/// of threads that Rayon is allowed to use.
/// If you pass None or don't call this function at all, Rayon uses all
/// the cores available (determined via num_cpus::get())
/// This function panics if:
///  - you pass zero as the number of threads
///  - you call it multiple times and the arguments you pass are different
///    for each call
pub fn initialize(num_threads: Option<usize>) {
    let registry = thread_pool::get_registry(num_threads);
    if let Some(value) = num_threads {
        assert!(value == registry.num_threads());
        assert!(value > 0);
    }
    registry.wait_until_primed();
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

    thread_pool::get_registry(None).inject(&[&mut job_a, &mut job_b]);

    latch_a.wait();
    latch_b.wait();

    (result_a.unwrap(), result_b.unwrap())
}

pub struct ThreadPool {
    registry: Arc<Registry>
}

impl ThreadPool {
    /// Constructs a new thread pool
    /// If you pass None as arguments, the current number of cores are used
    /// as determinde via num_cpus::get()
    /// This function panics if you pass zero as the number of threads
    pub fn new(num_threads: Option<usize>) -> ThreadPool {
        if let Some(value) = num_threads {
            assert!(value > 0);
        }
        ThreadPool {
            registry: Registry::new(num_threads)
        }
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
