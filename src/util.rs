use std::mem;

pub fn leak<T>(b: Box<T>) -> &'static T {
    unsafe {
        mem::transmute(b)
    }
}
