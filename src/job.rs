use latch::Latch;
use std::any::Any;
use std::cell::UnsafeCell;
use std::mem;
use unwind;

pub enum JobResult<T> {
    None,
    Ok(T),
    Panic(Box<Any + Send>),
}

/// A `Job` is used to advertise work for other threads that they may
/// want to steal. In accordance with time honored tradition, jobs are
/// arranged in a deque, so that thieves can take from the top of the
/// deque while the main worker manages the bottom of the deque. This
/// deque is managed by the `thread_pool` module.
pub trait Job {
    unsafe fn execute(this: *const Self);
}

/// Effectively a Job trait object. Each JobRef **must** be executed
/// exactly once, or else data may leak.
///
/// Internally, we store the job's data in a `*const ()` pointer.  The
/// true type is something like `*const StackJob<...>`, but we hide
/// it. We also carry the "execute fn" from the `Job` trait.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct JobRef {
    pointer: *const (),
    execute_fn: unsafe fn(*const ()),
}

unsafe impl Send for JobRef {}
unsafe impl Sync for JobRef {}

impl JobRef {
    pub unsafe fn new<T>(data: *const T) -> JobRef
        where T: Job
    {
        let fn_ptr: unsafe fn(*const T) = <T as Job>::execute;

        // erase types:
        let fn_ptr: unsafe fn(*const ()) = mem::transmute(fn_ptr);
        let pointer = data as *const ();

        JobRef {
            pointer: pointer,
            execute_fn: fn_ptr,
        }
    }

    #[inline]
    pub unsafe fn execute(&self) {
        (self.execute_fn)(self.pointer)
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
    where F: FnOnce() -> R + Send
{
    pub fn new(func: F, latch: L) -> StackJob<L, F, R> {
        StackJob {
            latch: latch,
            func: UnsafeCell::new(Some(func)),
            result: UnsafeCell::new(JobResult::None),
        }
    }

    pub unsafe fn as_job_ref(&self) -> JobRef {
        JobRef::new(self)
    }

    pub unsafe fn run_inline(self) -> R {
        self.func.into_inner().unwrap()()
    }

    pub unsafe fn into_result(self) -> R {
        self.result.into_inner().into_return_value()
    }
}

impl<L: Latch, F, R> Job for StackJob<L, F, R>
    where F: FnOnce() -> R
{
    unsafe fn execute(this: *const Self) {
        let this = &*this;
        let abort = unwind::AbortIfPanic;
        let func = (*this.func.get()).take().unwrap();
        (*this.result.get()) = match unwind::halt_unwinding(|| func()) {
            Ok(x) => JobResult::Ok(x),
            Err(x) => JobResult::Panic(x),
        };
        this.latch.set();
        mem::forget(abort);
    }
}

/// Represents a job stored in the heap. Used to implement
/// `scope`. Unlike `StackJob`, when executed, `HeapJob` simply
/// invokes a closure, which then triggers the appropriate logic to
/// signal that the job executed.
///
/// (Probably `StackJob` should be refactored in a similar fashion.)
pub struct HeapJob<BODY>
    where BODY: FnOnce()
{
    job: UnsafeCell<Option<BODY>>,
}

impl<BODY> HeapJob<BODY>
    where BODY: FnOnce()
{
    pub fn new(func: BODY) -> Self {
        HeapJob { job: UnsafeCell::new(Some(func)) }
    }

    /// Creates a `JobRef` from this job -- note that this hides all
    /// lifetimes, so it is up to you to ensure that this JobRef
    /// doesn't outlive any data that it closes over.
    pub unsafe fn as_job_ref(self: Box<Self>) -> JobRef {
        let this: *const Self = mem::transmute(self);
        JobRef::new(this)
    }
}

impl<BODY> Job for HeapJob<BODY>
    where BODY: FnOnce()
{
    unsafe fn execute(this: *const Self) {
        let this: Box<Self> = mem::transmute(this);
        let job = (*this.job.get()).take().unwrap();
        job();
    }
}

impl<T> JobResult<T> {
    /// Convert the `JobResult` for a job that has finished (and hence
    /// its JobResult is populated) into its return value.
    ///
    /// NB. This will panic if the job panicked.
    pub fn into_return_value(self) -> T {
        match self {
            JobResult::None => unreachable!(),
            JobResult::Ok(x) => x,
            JobResult::Panic(x) => unwind::resume_unwinding(x),
        }
    }
}
