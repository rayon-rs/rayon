use super::{ParallelIterator, ExactParallelIterator, IntoParallelIterator, FromParallelIterator};
use std::collections::LinkedList;
use std::slice;
use std::sync::atomic::{AtomicUsize, Ordering};

mod consumer;
use self::consumer::CollectConsumer;

mod test;

/// Collects the results of the exact iterator into the specified vector.
pub fn collect_into<I, T>(mut pi: I, v: &mut Vec<T>)
    where I: ExactParallelIterator<Item = T>,
          T: Send
{
    let mut collect = Collect::new(v, pi.len());
    pi.drive(collect.as_consumer());
    collect.complete();
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
fn special_collect_into<I, T>(pi: I, len: usize, v: &mut Vec<T>)
    where I: ParallelIterator<Item = T>,
          T: Send
{
    let mut collect = Collect::new(v, len);
    pi.drive_unindexed(collect.as_consumer());
    collect.complete();
}


/// Manage the collection vector.
struct Collect<'c, T: Send + 'c> {
    writes: AtomicUsize,
    vec: &'c mut Vec<T>,
    len: usize,
}

impl<'c, T: Send + 'c> Collect<'c, T> {
    fn new(vec: &'c mut Vec<T>, len: usize) -> Self {
        vec.truncate(0); // clear any old data
        vec.reserve(len); // reserve enough space

        Collect {
            writes: AtomicUsize::new(0),
            vec: vec,
            len: len,
        }
    }

    /// Create a consumer on a slice of our memory.
    fn as_consumer(&mut self) -> CollectConsumer<T> {
        // Get a correct borrow, then extend it to the original length.
        let mut slice = self.vec.as_mut_slice();
        slice = unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr(), self.len) };
        CollectConsumer::new(&self.writes, slice)
    }

    /// Update the final vector length.
    fn complete(mut self) {
        unsafe {
            // Here, we assert that `v` is fully initialized. This is
            // checked by the following assert, which counts how many
            // total writes occurred. Since we know that the consumer
            // cannot have escaped from `drive` (by parametricity,
            // essentially), we know that any stores that will happen,
            // have happened. Unless some code is buggy, that means we
            // should have seen `len` total writes.
            let actual_writes = self.writes.load(Ordering::Relaxed);
            assert!(actual_writes == self.len,
                    "expected {} total writes, but got {}",
                    self.len,
                    actual_writes);
            self.vec.set_len(self.len);
        }
    }
}


/// Collect items from a parallel iterator into a freshly allocated vector.
impl<T> FromParallelIterator<T> for Vec<T>
    where T: Send
{
    fn from_par_iter<I>(par_iter: I) -> Self
        where I: IntoParallelIterator<Item = T>
    {
        // See the vec_collect benchmarks in rayon-demo for different strategies.
        let mut par_iter = par_iter.into_par_iter();
        match par_iter.opt_len() {
            Some(len) => {
                // When Rust gets specialization, call `par_iter.collect_into()`
                // for exact iterators.  Until then, `special_collect_into()` fakes
                // the same thing on the promise that `opt_len()` is accurate.
                let mut vec = vec![];
                super::collect::special_collect_into(par_iter, len, &mut vec);
                vec
            }
            None => {
                // This works like `combine`, but `Vec::append` is more efficient than `extend`.
                let list: LinkedList<_> = par_iter.fold(Vec::new, |mut vec, elem| {
                        vec.push(elem);
                        vec
                    })
                    .collect();

                let len = list.iter().map(Vec::len).sum();
                let start = Vec::with_capacity(len);
                list.into_iter().fold(start, |mut vec, mut sub| {
                    vec.append(&mut sub);
                    vec
                })
            }
        }
    }
}
