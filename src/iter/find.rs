use std::sync::atomic::{AtomicBool, Ordering};
use super::internal::*;
use super::len::*;
use super::*;

pub fn find<PAR_ITER, P>(pi: PAR_ITER, find_op: P) -> Option<PAR_ITER::Item>
    where PAR_ITER: ParallelIterator,
          P: Fn(&PAR_ITER::Item) -> bool + Sync
{
    let found = AtomicBool::new(false);
    let consumer = FindConsumer::new(&find_op, &found);
    pi.drive_unindexed(consumer)
}

struct FindConsumer<'p, P: 'p> {
    find_op: &'p P,
    found: &'p AtomicBool,
}

impl<'p, P> FindConsumer<'p, P> {
    fn new(find_op: &'p P, found: &'p AtomicBool) -> Self {
        FindConsumer {
            find_op: find_op,
            found: found,
        }
    }
}

impl<'p, ITEM, P: 'p> Consumer<ITEM> for FindConsumer<'p, P>
    where ITEM: Send,
          P: Fn(&ITEM) -> bool + Sync
{
    type Folder = FindFolder<'p, ITEM, P>;
    type Reducer = FindReducer;
    type Result = Option<ITEM>;

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, Self::Reducer) {
        (self.split_off_left(), self, FindReducer)
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


impl<'p, ITEM, P: 'p> UnindexedConsumer<ITEM> for FindConsumer<'p, P>
    where ITEM: Send,
          P: Fn(&ITEM) -> bool + Sync
{
    fn split_off_left(&self) -> Self {
        FindConsumer::new(self.find_op, self.found)
    }

    fn to_reducer(&self) -> Self::Reducer {
        FindReducer
    }
}


struct FindFolder<'p, ITEM, P: 'p> {
    find_op: &'p P,
    found: &'p AtomicBool,
    item: Option<ITEM>,
}

impl<'p, ITEM, P> Folder<ITEM> for FindFolder<'p, ITEM, P>
    where P: Fn(&ITEM) -> bool + 'p
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
