use super::super::noop::*;
use super::super::plumbing::*;
use std::marker::PhantomData;
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct CollectConsumer<'c, T: Send + 'c, U, F: MapFolder<U>, R> {
    /// Tracks how many items we successfully wrote. Used to guarantee
    /// safety in the face of panics or buggy parallel iterators.
    writes: &'c AtomicUsize,

    /// A slice covering the target memory, not yet initialized!
    target: &'c mut [T],

    marker: PhantomData<U>,
    map_folder: F,
    reducer: R,
}

pub struct CollectFolder<'c, T: Send + 'c, U, F: MapFolder<U>> {
    global_writes: &'c AtomicUsize,
    local_writes: usize,

    /// An iterator over the *uninitialized* target memory.
    target: slice::IterMut<'c, T>,

    marker: PhantomData<U>,
    map_folder: F,
}

impl<'c, T, U, F, R> CollectConsumer<'c, T, U, F, R>
where
    T: Send + 'c,
    F: MapFolder<U>,
    R: Reducer<F::Result>,
{
    /// The target memory is considered uninitialized, and will be
    /// overwritten without dropping anything.
    pub fn new(
        writes: &'c AtomicUsize,
        target: &'c mut [T],
        map_folder: F,
        reducer: R,
    ) -> CollectConsumer<'c, T, U, F, R> {
        CollectConsumer {
            writes: writes,
            target: target,
            marker: PhantomData,
            map_folder: map_folder,
            reducer: reducer,
        }
    }
}

impl<'c, T, U, F, R> Consumer<U> for CollectConsumer<'c, T, U, F, R>
where
    T: Send + 'c,
    U: Send,
    F: Clone + MapFolder<U, Output = T> + Send,
    F::Result: Send,
    R: Clone + Reducer<F::Result> + Send,
{
    type Folder = CollectFolder<'c, T, U, F>;
    type Reducer = R;
    type Result = F::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        // instances Read in the fields from `self` and then
        // forget `self`, since it has been legitimately consumed
        // (and not dropped during unwinding).
        let CollectConsumer {
            writes,
            target,
            map_folder,
            reducer,
            ..
        } = self;

        // Produce new consumers. Normal slicing ensures that the
        // memory range given to each consumer is disjoint.
        let (left, right) = target.split_at_mut(index);
        (
            CollectConsumer::new(writes, left, map_folder.clone(), reducer.clone()),
            CollectConsumer::new(writes, right, map_folder, reducer.clone()),
            reducer,
        )
    }

    fn into_folder(self) -> CollectFolder<'c, T, U, F> {
        CollectFolder {
            global_writes: self.writes,
            local_writes: 0,
            target: self.target.into_iter(),
            marker: PhantomData,
            map_folder: self.map_folder,
        }
    }

    fn full(&self) -> bool {
        false
    }
}

impl<'c, T, U, F> Folder<U> for CollectFolder<'c, T, U, F>
where
    T: Send + 'c,
    F: MapFolder<U, Output = T>,
{
    type Result = F::Result;

    fn consume(mut self, item: U) -> Self {
        let (map_folder, item) = self.map_folder.consume(item);
        self.map_folder = map_folder;

        // Compute target pointer and write to it. Safe because the iterator
        // does all the bounds checking; we're only avoiding the target drop.
        let head = self
            .target
            .next()
            .expect("too many values pushed to consumer");
        unsafe {
            ptr::write(head, item);
        }

        self.local_writes += 1;
        self
    }

    fn complete(self) -> Self::Result {
        assert!(self.target.len() == 0, "too few values pushed to consumer");

        // track total values written
        self.global_writes
            .fetch_add(self.local_writes, Ordering::Relaxed);

        self.map_folder.complete()
    }

    fn full(&self) -> bool {
        false
    }
}

/// Pretend to be unindexed for `special_collect_into_vec`,
/// but we should never actually get used that way...
impl<'c, T: Send + 'c> UnindexedConsumer<T>
    for CollectConsumer<'c, T, T, NoopConsumer, NoopReducer>
{
    fn split_off_left(&self) -> Self {
        unreachable!("CollectConsumer must be indexed!")
    }

    fn to_reducer(&self) -> Self::Reducer {
        NoopReducer
    }
}
