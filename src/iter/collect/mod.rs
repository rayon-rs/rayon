use super::noop::*;
use super::plumbing::*;
use super::{IndexedParallelIterator, IntoParallelIterator, ParallelExtend, ParallelIterator};
use std::collections::LinkedList;
use std::slice;
use std::sync::atomic::{AtomicUsize, Ordering};

mod consumer;
use self::consumer::CollectConsumer;
use super::unzip::unzip_indexed;

mod test;

/// The `MapFolder` trait represents a "map fold" operation, where the "map"
/// part is encoded by the `consume` method taking `Self::Item` and returning
/// `T`, and where the "fold" part is encoded by `map` taking `&mut self` and
/// the `complete` method returning `Self::Result`.
pub trait MapFolder<Item>: Sized {
    /// The output returned when consuming an item.
    type Output: Send;

    /// The type of result that will ultimately be produced by the map folder.
    type Result: Send;

    /// Consume an item.
    fn consume(self, item: Item) -> (Self, Self::Output);

    /// Finish consuming items, produce final result.
    fn complete(self) -> Self::Result;
}

/// This is not directly public, but called by
/// `IndexedParallelIterator::map_collect_fold_reduce_into_vec_with`.
pub fn map_collect_fold_reduce_into_vec_with<'c, I, T, U, F, R>(
    pi: I,
    v: &'c mut Vec<T>,
    init: U,
    mapfold_op: F,
    reduce_op: R,
) -> U
where
    I: IndexedParallelIterator,
    T: Send + 'c,
    U: Clone + Send,
    F: Fn(U, I::Item) -> (U, T) + Sync + Send,
    R: Fn(U, U) -> U + Sync + Send,
{
    #[derive(Clone)]
    struct MapFoldCallback<U, F> {
        result: U,
        mapfold_op: F,
    }

    impl<'a, I, T, U, F> MapFolder<I> for MapFoldCallback<U, F>
    where
        T: Send,
        U: Send,
        F: Fn(U, I) -> (U, T) + Send,
    {
        type Output = T;
        type Result = U;

        fn consume(self, item: I) -> (Self, Self::Output) {
            let MapFoldCallback { result, mapfold_op } = self;
            let (result, output) = mapfold_op(result, item);
            (MapFoldCallback { result, mapfold_op }, output)
        }

        fn complete(self) -> Self::Result {
            self.result
        }
    }

    #[derive(Clone)]
    struct ReducerCallback<R>(R);

    impl<U, R> Reducer<U> for ReducerCallback<R>
    where
        R: FnOnce(U, U) -> U,
    {
        fn reduce(self, left: U, right: U) -> U {
            (self.0)(left, right)
        }
    }

    map_collect_fold_reduce_into_vec(
        pi,
        v,
        MapFoldCallback {
            result: init,
            mapfold_op: &mapfold_op,
        },
        ReducerCallback(&reduce_op),
    )
}

fn map_collect_fold_reduce_into_vec<'c, I, T, F, R>(
    pi: I,
    v: &'c mut Vec<T>,
    map_folder: F,
    reducer: R,
) -> F::Result
where
    I: IndexedParallelIterator,
    T: Send + 'c,
    F: Clone + MapFolder<I::Item, Output = T> + Send,
    R: Clone + Reducer<F::Result> + Send,
{
    v.truncate(0); // clear any old data
    let mut collect = Collect::new(v, pi.len());
    let result = pi.drive(collect.as_consumer(map_folder, reducer));
    collect.complete();
    result
}

/// Collects the results of the exact iterator into the specified vector.
///
/// This is not directly public, but called by `IndexedParallelIterator::collect_into_vec`.
pub fn collect_into_vec<I, T>(pi: I, v: &mut Vec<T>)
where
    I: IndexedParallelIterator<Item = T>,
    T: Send,
{
    map_collect_fold_reduce_into_vec(pi, v, NoopConsumer, NoopReducer)
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
    pi.drive_unindexed(collect.as_noop_consumer());
    collect.complete();
}

/// Unzips the results of the exact iterator into the specified vectors.
///
/// This is not directly public, but called by `IndexedParallelIterator::unzip_into_vecs`.
pub fn unzip_into_vecs<I, A, B>(pi: I, left: &mut Vec<A>, right: &mut Vec<B>)
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

    unzip_indexed(pi, left.as_noop_consumer(), right.as_noop_consumer());

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
            vec: vec,
            len: len,
        }
    }

    fn as_noop_consumer(&mut self) -> CollectConsumer<T, T, NoopConsumer, NoopReducer> {
        self.as_consumer(NoopConsumer, NoopReducer)
    }

    /// Create a consumer on a slice of our memory.
    fn as_consumer<'a, U, F, R>(
        &'a mut self,
        map_folder: F,
        reducer: R,
    ) -> CollectConsumer<'a, T, U, F, R>
    where
        F: MapFolder<U, Output = T>,
        R: Reducer<F::Result>,
    {
        // Reserve the new space.
        self.vec.reserve(self.len);

        // Get a correct borrow, then extend it for the newly added length.
        let start = self.vec.len();
        let mut slice = &mut self.vec[start..];
        slice = unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr(), self.len) };
        CollectConsumer::new(&self.writes, slice, map_folder, reducer)
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
                let list: LinkedList<_> = par_iter
                    .fold(Vec::new, |mut vec, elem| {
                        vec.push(elem);
                        vec
                    })
                    .map(|vec| {
                        let mut list = LinkedList::new();
                        list.push_back(vec);
                        list
                    })
                    .reduce(LinkedList::new, |mut list1, mut list2| {
                        list1.append(&mut list2);
                        list1
                    });

                self.reserve(list.iter().map(Vec::len).sum());
                for mut vec in list {
                    self.append(&mut vec);
                }
            }
        }
    }
}
