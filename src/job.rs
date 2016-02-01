use latch::Latch;
use std::mem;

/// A `Job` is used to advertise work for other threads that they may
/// want to steal. In accordance with time honored tradition, jobs are
/// arranged in a deque, so that thieves can take from the top of the
/// deque while the main worker manages the bottom of the deque. This
/// deque is managed by the `thread_pool` module.
pub struct Job<L:Latch> {
    /// code to execute (if job is stolen)
    code: *mut Code,

    /// latch to signal once execution is done (if job is stolen)
    latch: *mut L,
}

impl<L:Latch> Job<L> {
    pub unsafe fn new<'a>(code: *mut (Code+'a), latch: *mut L) -> Job<L> {
        let code: *mut Code = mem::transmute(code);
        Job {
            code: code,
            latch: latch
        }
    }

    #[allow(unused_variables)]
    pub unsafe fn execute(&mut self) {
        // Use a guard here to ensure that the latch is always set, even if the
        // function panics. The panic will be propagated since the Option is
        // still None when it is unwrapped.
        struct LatchGuard<L:Latch>(*mut L);
        impl<L:Latch> Drop for LatchGuard<L> {
            fn drop(&mut self) {
                unsafe { (*self.0).set(); }
            }
        }
        let guard = LatchGuard(self.latch);
        (*self.code).execute();
    }
}

pub trait Code {
    unsafe fn execute(&mut self);
}

pub struct CodeImpl<F,R> {
    func: Option<F>,
    dest: *mut Option<R>,
}

impl<F,R> CodeImpl<F,R>
    where F: FnOnce() -> R
{
    pub fn new(func: F, dest: *mut Option<R>) -> CodeImpl<F,R> {
        CodeImpl { func: Some(func), dest: dest }
    }
}

impl<F,R> Code for CodeImpl<F,R>
    where F: FnOnce() -> R
{
    unsafe fn execute(&mut self) {
        let func = self.func.take().unwrap();
        *self.dest = Some(func())
    }
}
