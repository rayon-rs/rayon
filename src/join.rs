use latch::{LockLatch, SpinLatch};
#[allow(unused_imports)]
use log::Event::*;
use job::StackJob;
use thread_pool::{self, WorkerThread};
use std::mem;
use unwind;

/// The `join` function takes two closures and *potentially* runs them
/// in parallel. It returns a pair of the results from those closures.
/// However, the call to `join` incurs low overhead and is much
/// different compared to spawning two separate threads.
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

