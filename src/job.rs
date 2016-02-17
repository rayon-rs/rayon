use latch::Latch;
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
///
/// Note that we don't use a trait here due to `&mut` aliasing issues. A thief
/// needs an `&mut` to access the func and result fields, while the original
/// thread uses a `&` to access the latch. We solve this by simply using a raw
/// pointer which allows us to get an &mut for func and result while getting a
/// `&` for the latch.
#[derive(Copy, Clone)]
pub struct Job {
    pub execute: unsafe fn(*mut ()),
    pub data: *mut (),
}
unsafe impl Send for Job { }
unsafe impl Sync for Job { }

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

    pub unsafe fn as_job(&mut self) -> Job {
        unsafe fn execute<L:Latch,F,R>(data: *mut ())
            where F: FnOnce() -> R
        {
            let job = data as *mut JobImpl<L,F,R>;

            // Use a guard here to ensure that the latch is always set, even if
            // the function panics.
            struct PanicGuard<'a,L:Latch + 'a>(&'a L);
            impl<'a,L:Latch> Drop for PanicGuard<'a,L> {
                fn drop(&mut self) {
                    self.0.set();
                }
            }
            let _guard = PanicGuard(&(*job).latch);
            let func = (*job).func.take().unwrap();
            (*job).result = JobImpl::<L,F,R>::run_result(func);
        }

        Job {
            execute: execute::<L,F,R>,
            data: self as *mut _ as *mut (),
        }
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
        // We assert that func is recover-safe since it doesn't touch any of
        // our data and we will be propagating the panic back to the user at
        // the join() call.
        // FIXME(rust-lang/rust#31728) Use into_inner() instead
        let mut wrapper = AssertRecoverSafe::new(Some(func));
        match panic::recover(move || (*wrapper).take().unwrap()()) {
            Ok(x) => JobResult::Ok(x),
            Err(x) => JobResult::Panic(x),
        }
    }
}
