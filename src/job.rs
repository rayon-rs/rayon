use latch::Latch;
#[cfg(feature = "nightly")]
use std::any::Any;
#[cfg(feature = "nightly")]
use std::panic::{self, AssertRecoverSafe};
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
    unsafe fn execute(&self);
    unsafe fn abort(&self);
}

#[derive(Copy, Clone)]
pub struct JobRef(pub *const Job);
unsafe impl Send for JobRef {}
unsafe impl Sync for JobRef {}

pub struct JobImpl<L:Latch,F,R> {
    pub latch: L,
    func: UnsafeCell<Option<F>>,
    result: UnsafeCell<JobResult<R>>,
}

impl<L:Latch,F,R> JobImpl<L,F,R>
    where F: FnOnce() -> R
{
    pub fn new(func: F, latch: L) -> JobImpl<L,F,R> {
        JobImpl {
            latch: latch,
            func: UnsafeCell::new(Some(func)),
            result: UnsafeCell::new(JobResult::None),
        }
    }

    pub unsafe fn as_job_ref(&self) -> JobRef {
        let job_ref: *const (Job + 'static) = mem::transmute(self as *const Job);
        JobRef(job_ref)
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
            JobResult::Panic(x) => panic::propagate(x),
        }
    }

    #[cfg(not(feature = "nightly"))]
    fn run_result(func: F) -> JobResult<R> {
        JobResult::Ok(func())
    }

    #[cfg(feature = "nightly")]
    fn run_result(func: F) -> JobResult<R> {
        // We assert that func is recover-safe since it doesn't touch any of
        // our data and we will be propagating the panic back to the user at
        // the join() call.
        let wrapper = AssertRecoverSafe::new(func);
        match panic::recover(move || wrapper.into_inner()()) {
            Ok(x) => JobResult::Ok(x),
            Err(x) => JobResult::Panic(x),
        }
    }
}

impl<L:Latch,F,R> Job for JobImpl<L,F,R>
    where F: FnOnce() -> R
{
    unsafe fn execute(&self) {
        // Use a guard here to ensure that the latch is always set, even if
        // the function panics.
        struct PanicGuard<'a,L:Latch + 'a>(&'a L);
        impl<'a,L:Latch> Drop for PanicGuard<'a,L> {
            fn drop(&mut self) {
                self.0.set();
            }
        }

        let _guard = PanicGuard(&self.latch);
        let func = (*self.func.get()).take().unwrap();
        (*self.result.get()) = Self::run_result(func);
    }

    unsafe fn abort(&self) {
        // Just set the latch and leave the result as None
        self.latch.set();
    }
}
