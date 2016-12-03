use std::sync::atomic::{AtomicBool, Ordering};
use super::internal::*;
use super::len::*;
use super::*;

pub fn find<PAR_ITER, FIND_OP>(pi: PAR_ITER, find_op: FIND_OP) -> Option<PAR_ITER::Item>
    where PAR_ITER: ParallelIterator,
          FIND_OP: Fn(&PAR_ITER::Item) -> bool + Sync
{
    let found = AtomicBool::new(false);
    let consumer = FindConsumer::new(&find_op, &found);
    pi.drive_unindexed(consumer)
}

struct FindConsumer<'f, FIND_OP: 'f> {
    find_op: &'f FIND_OP,
    found: &'f AtomicBool,
}

impl<'f, FIND_OP> FindConsumer<'f, FIND_OP> {
    fn new(find_op: &'f FIND_OP, found: &'f AtomicBool) -> Self {
        FindConsumer {
            find_op: find_op,
            found: found,
        }
    }
}

impl<'f, ITEM, FIND_OP: 'f> Consumer<ITEM> for FindConsumer<'f, FIND_OP>
    where ITEM: Send,
          FIND_OP: Fn(&ITEM) -> bool + Sync
{
    type Folder = FindFolder<'f, ITEM, FIND_OP>;
    type Reducer = FindReducer;
    type Result = Option<ITEM>;

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, Self::Reducer) {
        (self.split_off(), self, FindReducer)
    }

    fn into_folder(self) -> Self::Folder {
        FindFolder {
            find_op: self.find_op,
            found: self.found,
            item: None,
        }
    }

    fn full(&self) -> bool {
        self.found.load(Ordering::Relaxed)
    }
}


impl<'f, ITEM, FIND_OP: 'f> UnindexedConsumer<ITEM> for FindConsumer<'f, FIND_OP>
    where ITEM: Send,
          FIND_OP: Fn(&ITEM) -> bool + Sync
{
    fn split_off(&self) -> Self {
        FindConsumer::new(self.find_op, self.found)
    }

    fn to_reducer(&self) -> Self::Reducer {
        FindReducer
    }
}


struct FindFolder<'f, ITEM, FIND_OP: 'f> {
    find_op: &'f FIND_OP,
    found: &'f AtomicBool,
    item: Option<ITEM>,
}

impl<'f, ITEM, FIND_OP> Folder<ITEM> for FindFolder<'f, ITEM, FIND_OP>
    where FIND_OP: Fn(&ITEM) -> bool + 'f
{
    type Result = Option<ITEM>;

    fn consume(mut self, item: ITEM) -> Self {
        if (self.find_op)(&item) {
            self.found.store(true, Ordering::Relaxed);
            self.item = Some(item);
        }
        self
    }

    fn complete(self) -> Self::Result {
        self.item
    }

    fn full(&self) -> bool {
        self.found.load(Ordering::Relaxed)
    }
}


struct FindReducer;

impl<ITEM> Reducer<Option<ITEM>> for FindReducer {
    fn reduce(self, left: Option<ITEM>, right: Option<ITEM>) -> Option<ITEM> {
        left.or(right)
    }
}
