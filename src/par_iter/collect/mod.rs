use super::{ParallelIterator, ExactParallelIterator};
use std::isize;
use std::sync::atomic::{AtomicUsize, Ordering};

mod consumer;
use self::consumer::CollectConsumer;

/// Collects the results of the exact iterator into the specified vector.
pub fn collect_into<PAR_ITER, T>(mut pi: PAR_ITER, v: &mut Vec<T>)
    where PAR_ITER: ExactParallelIterator<Item = T>,
          T: Send
{
    let len = pi.len();
    special_collect_into(pi, len, v);
}

/// Collects the results of the iterator into the specified vector.
///
/// Technically, this only works for `ExactParallelIterator`, but we're faking a
/// bit of specialization here until Rust can do that natively.  Callers are
/// using `opt_len` to find the length before calling this, and only exact
/// iterators will return anything but `None` there.
///
/// Since the type system doesn't understand that contract, we have to allow
/// *any* `ParallelIterator` here, and `CollectConsumer` has to also implement
/// `UnindexedConsumer`.  That implementation panics `unreachable!` in case
/// there's a bug where we actually do try to use this unindexed.
#[doc(hidden)]
pub fn special_collect_into<PAR_ITER, T>(pi: PAR_ITER, len: usize, v: &mut Vec<T>)
    where PAR_ITER: ParallelIterator<Item = T>,
          T: Send
{
    assert!(len < isize::MAX as usize);

    v.truncate(0); // clear any old data
    v.reserve(len); // reserve enough space
    let target = v.as_mut_ptr(); // get a raw ptr
    let writes = AtomicUsize::new(0);
    let consumer = unsafe {
        // assert that target..target+len is unique and writable
        CollectConsumer::new(&writes, target, len)
    };
    pi.drive_unindexed(consumer);

    unsafe {
        // Here, we assert that `v` is fully initialized. This is
        // checked by the following assert, which counts how many
        // total writes occurred. Since we know that the consumer
        // cannot have escaped from `drive` (by parametricity,
        // essentially), we know that any stores that will happen,
        // have happened. Unless some code is buggy, that means we
        // should have seen `len` total writes.
        let actual_writes = writes.load(Ordering::Relaxed);
        assert!(actual_writes == len,
                "expected {} total writes, but got {}",
                len,
                actual_writes);
        v.set_len(len);
    }
}
