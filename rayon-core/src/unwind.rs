//! Package up unwind recovery. Note that if you are in some sensitive
//! place, you can use the `AbortIfPanic` helper to protect against
//! accidental panics in the rayon code itself.

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::io::stderr;
use std::io::prelude::*;
use std::thread;

/// Executes `f` and captures any panic, translating that panic into a
/// `Err` result. The assumption is that any panic will be propagated
/// later with `resume_unwinding`, and hence `f` can be treated as
/// exception safe.
pub fn halt_unwinding<F, R>(func: F) -> thread::Result<R>
    where F: FnOnce() -> R
{
    panic::catch_unwind(AssertUnwindSafe(func))
}

pub fn resume_unwinding(payload: Box<Any + Send>) -> ! {
    panic::resume_unwind(payload)
}

pub struct AbortIfPanic;

fn aborting() {
    let _ = writeln!(&mut stderr(), "Rayon: detected unexpected panic; aborting");
}

impl Drop for AbortIfPanic {
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    fn drop(&mut self) {
        aborting();
        ::std::process::abort(); // stable in rust 1.17
    }

    #[cfg(not(all(target_arch = "wasm32", not(target_os = "emscripten"))))]
    fn drop(&mut self) {
        aborting();
        unsafe {
            ::libc::abort(); // used for compat before 1.17
        }
    }
}
