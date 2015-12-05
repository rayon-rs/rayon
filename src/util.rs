use std::mem;
use std::sync::Arc;

pub fn leak<T>(b: Arc<T>) -> &'static T {
    unsafe {
        let p: *const T = &*b;
        mem::forget(b); // leak our reference, so that `b` is never freed
        &*p
    }
}
