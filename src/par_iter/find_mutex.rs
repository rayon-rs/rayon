use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use super::*;
use super::len::*;
use super::internal::*;

pub fn find<PAR_ITER, FIND_OP>(pi: PAR_ITER, find_op: FIND_OP) -> Option<PAR_ITER::Item>
    where PAR_ITER: ParallelIterator,
          FIND_OP: Fn(&PAR_ITER::Item) -> bool + Sync,
{
    let finder = Find {
        find_op: find_op,
        found: AtomicBool::new(false),
        result: Mutex::new(None),
    };
    pi.drive_unindexed(&finder);
    finder.result.into_inner().unwrap()
}

struct Find<ITEM: Send, FIND_OP: Fn(&ITEM) -> bool + Sync> {
    find_op: FIND_OP,
    found: AtomicBool,
    result: Mutex<Option<ITEM>>,
}

impl<'f, ITEM, FIND_OP> Consumer<ITEM> for &'f Find<ITEM, FIND_OP>
    where ITEM: Send, FIND_OP: Fn(&ITEM) -> bool + Sync
{
    type Folder = Self;
    type Reducer = NoopReducer;
    type Result = ();

    fn cost(&mut self, cost: f64) -> f64 {
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, Self::Reducer) {
        (self, self, NoopReducer)
    }

    fn into_folder(self) -> Self::Folder {
        self
    }

    fn should_continue(&self) -> bool {
        self.found.load(Ordering::Relaxed)
    }
}


impl<'f, ITEM, FIND_OP> UnindexedConsumer<ITEM> for &'f Find<ITEM, FIND_OP>
    where ITEM: Send, FIND_OP: Fn(&ITEM) -> bool + Sync
{
    fn split_off(&self) -> Self {
        self
    }

    fn to_reducer(&self) -> Self::Reducer {
        NoopReducer
    }
}


impl<'f, ITEM, FIND_OP> Folder<ITEM> for &'f Find<ITEM, FIND_OP>
    where ITEM: Send, FIND_OP: Fn(&ITEM) -> bool + Sync
{
    type Result = ();

    fn consume(self, item: ITEM) -> Self {
        if (self.find_op)(&item) {
            self.found.store(true, Ordering::Relaxed);
            if let Ok(mut guard) = self.result.try_lock() {
                if guard.is_none() {
                    *guard = Some(item);
                }
            }
        }
        self
    }

    fn complete(self) -> Self::Result {
    }

    fn should_continue(&self) -> bool {
        self.found.load(Ordering::Relaxed)
    }
}
