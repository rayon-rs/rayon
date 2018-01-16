use latch::{LatchProbe};
use log::Event::*;
use job::StackJob;
use registry::{self, WorkerThread};
use unwind;
use fiber::{Waitable, Fiber, ResumeAction};

use FnContext;
use fiber::SingleWaiterLatch;
use PoisonedJob;

#[cfg(test)]
mod test;

/// The `join` function takes two closures and *potentially* runs them
/// in parallel. It returns a pair of the results from those closures.
///
/// Conceptually, calling `join()` is similar to spawning two threads,
/// one executing each of the two closures. However, the
/// implementation is quite different and incurs very low
/// overhead. The underlying technique is called "work stealing": the
/// Rayon runtime uses a fixed pool of worker threads and attempts to
/// only execute code in parallel when there are idle CPUs to handle
/// it.
///
/// When `join` is called from outside the thread pool, the calling
/// thread will block while the closures execute in the pool.  When
/// `join` is called within the pool, the calling thread still actively
/// participates in the thread pool. It will begin by executing closure
/// A (on the current thread). While it is doing that, it will advertise
/// closure B as being available for other threads to execute. Once closure A
/// has completed, the current thread will try to execute closure B;
/// if however closure B has been stolen, then it will look for other work
/// while waiting for the thief to fully execute closure B. (This is the
/// typical work-stealing strategy).
///
/// ### Warning about blocking I/O
///
/// The assumption is that the closures given to `join()` are
/// CPU-bound tasks that do not perform I/O or other blocking
/// operations. If you do perform I/O, and that I/O should block
/// (e.g., waiting for a network request), the overall performance may
/// be poor.  Moreover, if you cause one closure to be blocked waiting
/// on another (for example, using a channel), that could lead to a
/// deadlock.
///
/// ### Panics
///
/// No matter what happens, both closures will always be executed.  If
/// a single closure panics, whether it be the first or second
/// closure, that panic will be propagated and hence `join()` will
/// panic with the same panic value. If both closures panic, `join()`
/// will panic with the panic value from the first closure.
pub fn join<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
    where A: FnOnce() -> RA + Send,
          B: FnOnce() -> RB + Send,
          RA: Send,
          RB: Send
{
    join_context(|_| oper_a(), |_| oper_b())
}

/// The `join_context` function is identical to `join`, except the closures
/// have a parameter that provides context for the way the closure has been
/// called, especially indicating whether they're executing on a different
/// thread than where `join_context` was called.  This will occur if the second
/// job is stolen by a different thread, or if `join_context` was called from
/// outside the thread pool to begin with.
pub fn join_context<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
    where A: FnOnce(FnContext) -> RA + Send,
          B: FnOnce(FnContext) -> RB + Send,
          RA: Send,
          RB: Send
{
    registry::in_worker(|worker_thread, injected| unsafe {
        log!(Join { worker: worker_thread.index() });

        // Create virtual wrapper for task b; this all has to be
        // done here so that the stack frame can keep it all live
        // long enough.
        let job_b = StackJob::new(|migrated| oper_b(FnContext::new(migrated)),
                                  SingleWaiterLatch::new());
        let job_b_ref = job_b.as_job_ref();
        job_b.latch.start();
        worker_thread.push(job_b_ref);

        // Execute task a; hopefully b gets stolen in the meantime.
        let status_a = unwind::halt_unwinding(move || oper_a(FnContext::new(injected)));
        let result_a = match status_a {
            Ok(v) => v,
            Err(err) => {
                // If job A panics, we still cannot return until we are sure that job
                // B is complete. This is because it may contain references into the
                // enclosing stack frame(s).
                worker_thread.wait_until(&job_b.latch);
                debug_assert!(job_b.latch.probe());
                if err.is::<PoisonedJob>() {
                    // Job A was poisoned, so unwind the panic of Job B if it exists
                    job_b.into_result();
                }
                unwind::resume_unwinding(err)
            },
        };

        // Now that task A has finished, try to pop job B from the
        // local stack.  It may already have been popped by job A; it
        // may also have been stolen. There may also be some tasks
        // pushed on top of it in the stack, and we will have to pop
        // those off to get to it.
        while !job_b.latch.probe() {
            if let Some(job) = worker_thread.take_local_job() {
                if job == job_b_ref {
                    // Found it! Let's run it.
                    //
                    // Note that this could panic, but it's ok if we unwind here.
                    log!(PoppedRhs { worker: worker_thread.index() });
                    let result_b = job_b.run_inline(injected);
                    return (result_a, result_b);
                } else {
                    log!(PoppedJob { worker: worker_thread.index() });

                    struct ResumeAgain;

                    impl Waitable for ResumeAgain {
                        fn complete(&self, _worker_thread: &WorkerThread) -> bool {
                            panic!()
                        }

                        fn await(&self, worker_thread: &WorkerThread, waiter: Fiber, _tlv: usize) {
                            let worker_index = worker_thread.index();
                            worker_thread.registry.resume_fiber(worker_index, waiter);
                        }
                    }

                    worker_thread.execute(job, ResumeAction::StoreInWaitable(&ResumeAgain, 0));
                }
            } else {
                // CHECK: This can steal our job back to this thread
                // Local deque is empty. Time to steal from other
                // threads.
                log!(LostJob { worker: worker_thread.index() });
                worker_thread.wait_until(&job_b.latch);
                debug_assert!(job_b.latch.probe());
                break;
            }
        }

        return (result_a, job_b.into_result());
    })
}