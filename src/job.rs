use latch::Latch;
use std::mem;
#[cfg(feature = "nightly")]
use std::any::Any;
#[cfg(feature = "nightly")]
use std::panic::{self, AssertRecoverSafe};

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
pub trait Job<L:Latch> {
    unsafe fn execute(&mut self);
}

pub struct JobImpl<L:Latch,F,R> {
    pub latch: L,
    func: Option<F>,
    result: JobResult<R>,
}

impl<L:Latch,F,R> JobImpl<L,F,R>
    where F: FnOnce() -> R
{
    pub fn new(func: F, latch: L) -> JobImpl<L,F,R> {
        JobImpl {
            latch: latch,
            func: Some(func),
            result: JobResult::None,
        }
    }

    pub unsafe fn as_static(&mut self) -> *mut Job<L> {
        mem::transmute(self as *mut Job<L>)
    }

    pub fn run_inline(self) -> R {
        self.func.unwrap()()
    }

    #[cfg(not(feature = "nightly"))]
    pub fn into_result(self) -> R {
        match self.result {
            JobResult::None => panic!("job function panicked"),
            JobResult::Ok(x) => x,
        }
    }

    #[cfg(feature = "nightly")]
    pub fn into_result(self) -> R {
        match self.result {
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
        // RecoverSafe is a bit ugly to deal with...
        // Ideally this would just be panic::recover(func)
        let mut wrapper = AssertRecoverSafe::new(Some(func));
        match panic::recover(move || (*wrapper).take().unwrap()()) {
            Ok(x) => JobResult::Ok(x),
            Err(x) => JobResult::Panic(x),
        }
    }
}

impl<L:Latch,F,R> Job<L> for JobImpl<L,F,R>
    where F: FnOnce() -> R
{
    unsafe fn execute(&mut self) {
        // Use a guard here to ensure that the latch is always set, even if the
        // function panics.
        struct PanicGuard<'a,L:Latch + 'a>(&'a L);
        impl<'a,L:Latch> Drop for PanicGuard<'a,L> {
            fn drop(&mut self) {
                self.0.set();
            }
        }
        let _guard = PanicGuard(&self.latch);
        let func = self.func.take().unwrap();
        self.result = Self::run_result(func);
    }
}
