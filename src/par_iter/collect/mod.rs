use super::ExactParallelIterator;
use std::isize;
use std::sync::atomic::{AtomicUsize, Ordering};

mod consumer;
use self::consumer::CollectConsumer;

pub fn collect_into<PAR_ITER,T>(mut pi: PAR_ITER, v: &mut Vec<T>)
    where PAR_ITER: ExactParallelIterator<Item=T>,
          PAR_ITER: ExactParallelIterator,
          T: Send,
{
    let len = pi.len();
    assert!(len < isize::MAX as usize);

    v.truncate(0); // clear any old data
    v.reserve(len); // reserve enough space
    let target = v.as_mut_ptr(); // get a raw ptr
    let writes = AtomicUsize::new(0);
    let consumer = unsafe { // assert that target..target+len is unique and writable
        CollectConsumer::new(&writes, target, len)
    };
    pi.drive(consumer);

    unsafe {
        // Here, we assert that `v` is fully initialized. This is
        // checked by the following assert, which counts how many
        // total writes occurred. Since we know that the consumer
        // cannot have escaped from `drive` (by parametricity,
        // essentially), we know that any stores that will happen,
        // have happened. Unless some code is buggy, that means we
        // should have seen `len` total writes.
        let actual_writes = writes.load(Ordering::Relaxed);
        assert!(actual_writes == len, "expected {} total writes, but got {}", len, actual_writes);
        v.set_len(len);
    }
}

