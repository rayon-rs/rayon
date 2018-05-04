//! Allows access to the Rayon's thread local value
//! which is preserved when moving jobs across threads

use std::cell::Cell;

thread_local!(pub(crate) static TLV: Cell<usize> = Cell::new(0));

/// Sets the current thread-local value to `value` inside the closure.
/// The old value is restored when the closure ends
pub fn with<F: FnOnce() -> R, R>(value: usize, f: F) -> R {
    struct Reset(usize);
    impl Drop for Reset {
        fn drop(&mut self) {
            TLV.with(|tlv| tlv.set(self.0));
        }
    }
    let _reset = Reset(get());
    TLV.with(|tlv| tlv.set(value));
    f()
}

/// Sets the current thread-local value
pub fn set(value: usize) {
    TLV.with(|tlv| tlv.set(value));
}

/// Returns the current thread-local value
pub fn get() -> usize {
    TLV.with(|tlv| tlv.get())
}
