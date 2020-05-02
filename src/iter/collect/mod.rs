use super::{IndexedParallelIterator, IntoParallelIterator, ParallelExtend, ParallelIterator};
use std::slice;

mod consumer;
use self::consumer::CollectConsumer;
use self::consumer::CollectResult;
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
    let len = pi.len();
    Collect::new(v, len).with_consumer(|consumer| pi.drive(consumer));
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
    Collect::new(v, len).with_consumer(|consumer| pi.drive_unindexed(consumer));
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
    Collect::new(right, len).with_consumer(|right_consumer| {
        let mut right_result = None;
        Collect::new(left, len).with_consumer(|left_consumer| {
            let (left_r, right_r) = unzip_indexed(pi, left_consumer, right_consumer);
            right_result = Some(right_r);
            left_r
        });
        right_result.unwrap()
    });
}

/// Manage the collection vector.
struct Collect<'c, T: Send> {
    vec: &'c mut Vec<T>,
    len: usize,
}

impl<'c, T: Send + 'c> Collect<'c, T> {
    fn new(vec: &'c mut Vec<T>, len: usize) -> Self {
        Collect { vec, len }
    }

    /// Create a consumer on the slice of memory we are collecting into.
    ///
    /// The consumer needs to be used inside the scope function, and the
    /// complete collect result passed back.
    ///
    /// This method will verify the collect result, and panic if the slice
    /// was not fully written into. Otherwise, in the successful case,
    /// the vector is complete with the collected result.
    fn with_consumer<F>(mut self, scope_fn: F)
    where
        F: FnOnce(CollectConsumer<T>) -> CollectResult<T>,
    {
        unsafe {
            let slice = Self::reserve_get_tail_slice(&mut self.vec, self.len);
            let result = scope_fn(CollectConsumer::new(slice));

            // The CollectResult represents a contiguous part of the
            // slice, that has been written to.
            // On unwind here, the CollectResult will be dropped.
            // If some producers on the way did not produce enough elements,
            // partial CollectResults may have been dropped without
            // being reduced to the final result, and we will see
            // that as the length coming up short.
            //
            // Here, we assert that `slice` is fully initialized. This is
            // checked by the following assert, which verifies if a
            // complete CollectResult was produced; if the length is
            // correct, it is necessarily covering the target slice.
            // Since we know that the consumer cannot have escaped from
            // `drive` (by parametricity, essentially), we know that any
            // stores that will happen, have happened. Unless some code is buggy,
            // that means we should have seen `len` total writes.
            let actual_writes = result.len();
            assert!(
                actual_writes == self.len,
                "expected {} total writes, but got {}",
                self.len,
                actual_writes
            );

            // Release the result's mutable borrow and "proxy ownership"
            // of the elements, before the vector takes it over.
            result.release_ownership();

            let new_len = self.vec.len() + self.len;
            self.vec.set_len(new_len);
        }
    }

    /// Reserve space for `len` more elements in the vector,
    /// and return a slice to the uninitialized tail of the vector
    ///
    /// Safety: The tail slice is uninitialized
    unsafe fn reserve_get_tail_slice(vec: &mut Vec<T>, len: usize) -> &mut [T] {
        // Reserve the new space.
        vec.reserve(len);

        // Get a correct borrow, then extend it for the newly added length.
        let start = vec.len();
        let slice = &mut vec[start..];
        slice::from_raw_parts_mut(slice.as_mut_ptr(), len)
    }
}

/// Extends a vector with items from a parallel iterator.
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
