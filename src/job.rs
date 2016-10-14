use latch::Latch;
#[cfg(feature = "nightly")]
use std::any::Any;
#[cfg(feature = "nightly")]
use std::panic::{self, AssertUnwindSafe};
use std::cell::UnsafeCell;
use std::mem;

enum JobResult<T> {
    None,
    Ok(T),
    #[cfg(feature = "nightly")]
    Panic(Box<Any + Send>),
}

/// A `Job` is used to advertise work for other threads that they may
/// want to steal. In accordance with time honored tradition, jobs are
/// arranged in a deque, so that thieves can take from the top of the
/// deque while the main worker manages the bottom of the deque. This
/// deque is managed by the `thread_pool` module.
pub trait Job {
    unsafe fn execute(&self, mode: JobMode);
}

pub enum JobMode {
    Execute,
    Abort,
}

#[derive(Copy, Clone)]
pub struct JobRef {
    pointer: *const Job
}

unsafe impl Send for JobRef {}
unsafe impl Sync for JobRef {}

impl JobRef {
    #[inline]
    pub unsafe fn execute(&self, mode: JobMode) {
        (*self.pointer).execute(mode);
    }
}

/// A job that will be owned by a stack slot. This means that when it
/// executes it need not free any heap data, the cleanup occurs when
/// the stack frame is later popped.
pub struct StackJob<L: Latch, F, R> {
    pub latch: L,
    func: UnsafeCell<Option<F>>,
    result: UnsafeCell<JobResult<R>>,
}

impl<L: Latch, F, R> StackJob<L, F, R>
    where F: FnOnce() -> R
{
    pub fn new(func: F, latch: L) -> StackJob<L, F, R> {
        StackJob {
            latch: latch,
            func: UnsafeCell::new(Some(func)),
            result: UnsafeCell::new(JobResult::None),
        }
    }

    pub unsafe fn as_job_ref(&self) -> JobRef {
        let job_ref: *const (Job + 'static) = mem::transmute(self as *const Job);
        JobRef { pointer: job_ref }
    }

    pub unsafe fn run_inline(self) -> R {
        self.func.into_inner().unwrap()()
    }

    #[cfg(not(feature = "nightly"))]
    pub unsafe fn into_result(self) -> R {
        match self.result.into_inner() {
            JobResult::None => panic!("job function panicked"),
            JobResult::Ok(x) => x,
        }
    }

    #[cfg(feature = "nightly")]
    pub unsafe fn into_result(self) -> R {
        match self.result.into_inner() {
            JobResult::None => unreachable!(),
            JobResult::Ok(x) => x,
            JobResult::Panic(x) => panic::resume_unwind(x),
        }
    }

    #[cfg(not(feature = "nightly"))]
    fn run_result(func: F) -> JobResult<R> {
        JobResult::Ok(func())
    }

    #[cfg(feature = "nightly")]
    fn run_result(func: F) -> JobResult<R> {
        // We assert that func is unwind-safe since it doesn't touch any of
        // our data and we will be propagating the panic back to the user at
        // the join() call.
        match panic::catch_unwind(AssertUnwindSafe(func)) {
            Ok(x) => JobResult::Ok(x),
            Err(x) => JobResult::Panic(x),
        }
    }
}

impl<L: Latch, F, R> Job for StackJob<L, F, R>
    where F: FnOnce() -> R
{
    unsafe fn execute(&self, mode: JobMode) {
        match mode {
            JobMode::Execute => {
                // Use a guard here to ensure that the latch is always set, even if
                // the function panics.
                struct PanicGuard<'a, L: Latch + 'a>(&'a L);
                impl<'a, L: Latch> Drop for PanicGuard<'a, L> {
                    fn drop(&mut self) {
                        self.0.set();
                    }
                }

                let _guard = PanicGuard(&self.latch);
                let func = (*self.func.get()).take().unwrap();
                (*self.result.get()) = Self::run_result(func);
            }
            JobMode::Abort => {
                self.latch.set();
            }
        }
    }
}
