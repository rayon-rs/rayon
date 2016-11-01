use std::sync::atomic::{AtomicUsize, Ordering};

use super::internal::*;
use super::len::*;
use super::ParallelIterator;

const SEARCHING: usize = 0;
const FOUND: usize = 1;

pub fn find_any<PAR_ITER, PREDICATE>(par_iter: PAR_ITER,
                                     predicate: &PREDICATE)
                                     -> Option<PAR_ITER::Item>
    where PAR_ITER: ParallelIterator,
          PREDICATE: Fn(&PAR_ITER::Item) -> bool + Sync,
{
    let flag: AtomicUsize = AtomicUsize::new(SEARCHING);
    let mut data: Option<PAR_ITER::Item> = None;

    // parallel execution: this will write to `data` for the
    // first thing we find (if any)
    {
        let data_ptr: &mut Option<PAR_ITER::Item> = &mut data;
        let consumer = FindAnyConsumer {
            flag: &flag,
            predicate: predicate,
            data_ptr: data_ptr,
        };
        par_iter.drive_unindexed(consumer);
    }

    data
}

struct FindAnyConsumer<'f, PREDICATE: 'f, ITEM> {
    flag: &'f AtomicUsize,
    predicate: &'f PREDICATE,
    data_ptr: *mut Option<ITEM>
}

impl<'f, PREDICATE, ITEM> Copy for FindAnyConsumer<'f, PREDICATE, ITEM> { }

impl<'f, PREDICATE, ITEM> Clone for FindAnyConsumer<'f, PREDICATE, ITEM> {
    fn clone(&self) -> Self {
        *self
    }
}

/// We need this because of the `*mut` that we are sending between
/// threads. We guarantee that it will still be valid because it
/// points into the stack of `find_any()` and `find_any()` is blocked
/// waiting for `FindAnyConsumer`.
unsafe impl<'f, PREDICATE, ITEM> Send for FindAnyConsumer<'f, PREDICATE, ITEM> { }

impl<'f, PREDICATE, ITEM> Consumer<ITEM> for FindAnyConsumer<'f, PREDICATE, ITEM>
    where PREDICATE: Fn(&ITEM) -> bool + Sync,
{
    type Folder = Self;
    type Reducer = NoopReducer;
    type Result = ();

    fn weighted(&self) -> bool {
        false
    }

    /// Cost to process `items` number of items.
    fn cost(&mut self, cost: f64) -> f64 {
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, NoopReducer) {
        (self, self, NoopReducer)
    }

    fn into_folder(self) -> Self::Folder {
        self
    }

    fn should_continue(&self) -> bool {
        self.flag.load(Ordering::Relaxed) == SEARCHING
    }
}

impl<'f, PREDICATE, ITEM> UnindexedConsumer<ITEM> for FindAnyConsumer<'f, PREDICATE, ITEM>
    where PREDICATE: Fn(&ITEM) -> bool + Sync,
{
    fn split_off(&self) -> Self {
        *self
    }

    fn to_reducer(&self) -> Self::Reducer {
        NoopReducer
    }
}

impl<'f, PREDICATE, ITEM> Folder<ITEM> for FindAnyConsumer<'f, PREDICATE, ITEM>
    where PREDICATE: Fn(&ITEM) -> bool + Sync,
{
    type Result = ();

    fn consume(self, item: ITEM) -> Self {
        if (self.predicate)(&item) {
            let state = self.flag.swap(FOUND, Ordering::Acquire);
            if state == SEARCHING {
                // I made the transition from SEARCHING to
                // FOUND. Therefore, I am responsible for storing a
                // value into the data slot. Moreover, we know the
                // slot is still valid because the `find_any` call is
                // blocked waiting for us to complete our execution.
                unsafe {
                    *self.data_ptr = Some(item);
                }
            }
        }
        self
    }

    fn complete(self) {
    }

    fn should_continue(&self) -> bool {
        self.flag.load(Ordering::Relaxed) == SEARCHING
    }
}

