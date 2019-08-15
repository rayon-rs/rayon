use super::{IndexedParallelIterator, IntoParallelIterator, ParallelExtend, ParallelIterator};
use std::slice;
use std::sync::atomic::{AtomicUsize, Ordering};

mod consumer;
use self::consumer::CollectConsumer;
use super::unzip::unzip_indexed;

mod test;

/// Collects the results of the exact iterator into the specified vector.
///
/// This is called by `IndexedParallelIterator::collect_into_vec`.
pub(super) fn collect_into_vec<I, T>(pi: I, v: &mut Vec<T>)
where
    I: IndexedParallelIterator<Item = T>,
    T: Send,
{
    v.truncate(0); // clear any old data
    let mut collect = Collect::new(v, pi.len());
    pi.drive(collect.as_consumer());
    collect.complete();
}

/// Collects the results of the iterator into the specified vector.
///
/// Technically, this only works for `IndexedParallelIterator`, but we're faking a
/// bit of specialization here until Rust can do that natively.  Callers are
/// using `opt_len` to find the length before calling this, and only exact
/// iterators will return anything but `None` there.
///
/// Since the type system doesn't understand that contract, we have to allow
/// *any* `ParallelIterator` here, and `CollectConsumer` has to also implement
/// `UnindexedConsumer`.  That implementation panics `unreachable!` in case
/// there's a bug where we actually do try to use this unindexed.
fn special_extend<I, T>(pi: I, len: usize, v: &mut Vec<T>)
where
    I: ParallelIterator<Item = T>,
    T: Send,
{
    let mut collect = Collect::new(v, len);
    pi.drive_unindexed(collect.as_consumer());
    collect.complete();
}

/// Unzips the results of the exact iterator into the specified vectors.
///
/// This is called by `IndexedParallelIterator::unzip_into_vecs`.
pub(super) fn unzip_into_vecs<I, A, B>(pi: I, left: &mut Vec<A>, right: &mut Vec<B>)
where
    I: IndexedParallelIterator<Item = (A, B)>,
    A: Send,
    B: Send,
{
    // clear any old data
    left.truncate(0);
    right.truncate(0);

    let len = pi.len();
    let mut left = Collect::new(left, len);
    let mut right = Collect::new(right, len);

    unzip_indexed(pi, left.as_consumer(), right.as_consumer());

    left.complete();
    right.complete();
}

/// Manage the collection vector.
struct Collect<'c, T: Send + 'c> {
    writes: AtomicUsize,
    vec: &'c mut Vec<T>,
    len: usize,
}

impl<'c, T: Send + 'c> Collect<'c, T> {
    fn new(vec: &'c mut Vec<T>, len: usize) -> Self {
        Collect {
            writes: AtomicUsize::new(0),
            vec,
            len,
        }
    }

    /// Create a consumer on a slice of our memory.
    fn as_consumer(&mut self) -> CollectConsumer<'_, T> {
        // Reserve the new space.
        self.vec.reserve(self.len);

        // Get a correct borrow, then extend it for the newly added length.
        let start = self.vec.len();
        let mut slice = &mut self.vec[start..];
        slice = unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr(), self.len) };
        CollectConsumer::new(&self.writes, slice)
    }

    /// Update the final vector length.
    fn complete(self) {
        unsafe {
            // Here, we assert that `v` is fully initialized. This is
            // checked by the following assert, which counts how many
            // total writes occurred. Since we know that the consumer
            // cannot have escaped from `drive` (by parametricity,
            // essentially), we know that any stores that will happen,
            // have happened. Unless some code is buggy, that means we
            // should have seen `len` total writes.
            let actual_writes = self.writes.load(Ordering::Relaxed);
            assert!(
                actual_writes == self.len,
                "expected {} total writes, but got {}",
                self.len,
                actual_writes
            );
            let new_len = self.vec.len() + self.len;
            self.vec.set_len(new_len);
        }
    }
}

/// Extend a vector with items from a parallel iterator.
impl<T> ParallelExtend<T> for Vec<T>
where
    T: Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = T>,
    {
        // See the vec_collect benchmarks in rayon-demo for different strategies.
        let par_iter = par_iter.into_par_iter();
        match par_iter.opt_len() {
            Some(len) => {
                // When Rust gets specialization, we can get here for indexed iterators
                // without relying on `opt_len`.  Until then, `special_extend()` fakes
                // an unindexed mode on the promise that `opt_len()` is accurate.
                special_extend(par_iter, len, self);
            }
            None => {
                // This works like `extend`, but `Vec::append` is more efficient.
                let list = super::extend::collect(par_iter);
                self.reserve(super::extend::len(&list));
                for mut vec in list {
                    self.append(&mut vec);
                }
            }
        }
    }
}
