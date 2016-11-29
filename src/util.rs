use std::mem;

pub fn leak<T>(v: T) -> &'static T {
    unsafe {
        let b = Box::new(v);
        let p: *const T = &*b;
        mem::forget(b); // leak our reference, so that `b` is never freed
        &*p
    }
}
