use latch::Latch;
#[allow(unused_imports)]
use log::Event::*;
use job::{Code, CodeImpl, Job};
use std::sync::Arc;
use thread_pool::{self, Registry, WorkerThread};

/// Initializes the Rayon threadpool. You don't normally need to do
/// this, as it happens automatically, but it is handy for
/// benchmarking purposes since it avoids initialization overhead in
/// the actual operations.
pub fn initialize() {
    let registry = thread_pool::get_registry();
    registry.wait_until_primed();
}

/// This is a debugging API not really intended for end users. It will
/// dump some performance statistics out using `println`.
pub fn dump_stats() {
    dump_stats!();
}

pub fn join<A,R_A,B,R_B>(oper_a: A,
                         oper_b: B)
                         -> (R_A, R_B)
    where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send,
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
        let mut latch_b = Latch::new();
        let mut job_b = Job::new(&mut code_b, &mut latch_b);
        (*worker_thread).push(&mut job_b);

        // execute task a; hopefully b gets stolen
        let result_a = oper_a();

        // if b was not stolen, do it ourselves, else wait for the thief to finish
        if (*worker_thread).pop(&mut job_b) {
            log!(PoppedJob { worker: (*worker_thread).index() });
            code_b.execute(); // not stolen, let's do it!
        } else {
            log!(LostJob { worker: (*worker_thread).index() });
            // TODO help out!
            latch_b.wait(); // stolen, wait for them to finish
        }

        // now result_b should be initialized
        (result_a, result_b.unwrap())
    }
}

#[inline(never)] // cold path
unsafe fn join_inject<A,R_A,B,R_B>(oper_a: A,
                                   oper_b: B)
                                   -> (R_A, R_B)
    where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send,
{
    let mut result_a = None;
    let mut code_a = CodeImpl::new(oper_a, &mut result_a);
    let mut latch_a = Latch::new();
    let mut job_a = Job::new(&mut code_a, &mut latch_a);

    let mut result_b = None;
    let mut code_b = CodeImpl::new(oper_b, &mut result_b);
    let mut latch_b = Latch::new();
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
    pub fn new() -> ThreadPool {
        ThreadPool {
            registry: Registry::new()
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
            let mut latch_a = Latch::new();
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
