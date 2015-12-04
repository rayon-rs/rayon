use latch::Latch;
#[allow(unused_imports)]
use log::Event::*;
use job::{Code, CodeImpl, Job};
use thread_pool::{self, WorkerThread};

/// Initializes the Rayon threadpool. You don't normally need to do
/// this, as it happens automatically, but it is handy for
/// benchmarking purposes since it avoids initialization overhead in
/// the actual operations.
pub fn initialize() {
    thread_pool::initialize();
}

pub fn join<A,R_A,B,R_B>(oper_a: A,
                         oper_b: B)
                         -> (R_A, R_B)
    where A: FnOnce() -> R_A + Send, B: FnOnce() -> R_B + Send,
{
    unsafe {
        let worker_thread = WorkerThread::current();
        log!(Join { worker: worker_thread });
        match worker_thread {
            Some(worker_thread) => {
                // create a home where we will write result of task b
                let mut result_b = None;

                // create virtual wrapper for task b; this all has to be
                // done here so that the stack frame can keep it all live
                // long enough
                let mut code_b = CodeImpl::new(oper_b, &mut result_b);
                let mut latch_b = Latch::new();
                let mut job_b = Job::new(&mut code_b, &mut latch_b);
                worker_thread.push(&mut job_b);

                // execute task a; hopefully b gets stolen
                let result_a = oper_a();

                // if b was not stolen, do it ourselves, else wait for the thief to finish
                if worker_thread.pop(&mut job_b) {
                    log!(PoppedJob { worker: worker_thread });
                    code_b.execute(); // not stolen, let's do it!
                } else {
                    log!(LostJob { worker: worker_thread });
                    // TODO help out!
                    latch_b.wait(); // stolen, wait for them to finish
                }

                // now result_b should be initialized
                (result_a, result_b.unwrap())
            }
            None => {
                join_inject(oper_a, oper_b)
            }
        }
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

    thread_pool::inject(&[&mut job_a, &mut job_b]);

    latch_a.wait();
    latch_b.wait();

    (result_a.unwrap(), result_b.unwrap())
}
