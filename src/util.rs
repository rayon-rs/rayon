use std::mem;

pub fn leak<T>(v: T) -> &'static T {
    unsafe {
        let b = Box::new(v);
        let p: *const T = &*b;
        mem::forget(b); // leak our reference, so that `b` is never freed
        &*p
    }
}

/// A dummy value that we can give to panic to indicate that we tried
/// to acquire a poisoned lock, e.g. during flat-combining. The
/// intention is for this to be treated specially by the panic
/// propagation code -- in cases where the code must pick a "best"
/// panic to propagate, it can ignore values of this type.
pub struct PoisonPanic;
