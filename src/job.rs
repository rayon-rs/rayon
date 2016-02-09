use latch::Latch;
use std::mem;

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
    result: Option<R>,
}

impl<L:Latch,F,R> JobImpl<L,F,R>
    where F: FnOnce() -> R
{
    pub fn new(func: F, latch: L) -> JobImpl<L,F,R> {
        JobImpl {
            latch: latch,
            func: Some(func),
            result: None,
        }
    }

    pub unsafe fn as_static(&mut self) -> *mut Job<L> {
        mem::transmute(self as *mut Job<L>)
    }

    pub fn run_inline(self) -> R {
        self.func.unwrap()()
    }

    pub fn into_result(self) -> R {
        self.result.expect("job function panicked")
    }
}

impl<L:Latch,F,R> Job<L> for JobImpl<L,F,R>
    where F: FnOnce() -> R
{
    unsafe fn execute(&mut self) {
        // Use a guard here to ensure that the latch is always set, even if the
        // function panics. The panic will be propagated since the Option is
        // still None when it is unwrapped.
        struct PanicGuard<'a,L:Latch + 'a>(&'a L);
        impl<'a,L:Latch> Drop for PanicGuard<'a,L> {
            fn drop(&mut self) {
                self.0.set();
            }
        }
        let _guard = PanicGuard(&self.latch);
        let func = self.func.take().unwrap();
        self.result = Some(func());
    }
}
