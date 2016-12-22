use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use super::internal::*;
use super::*;
use super::len::*;

// The consumer for find_first/find_last has fake indexes representing the lower
// and upper bounds of the "range" of data it consumes. This range does not
// correspond to indexes from the consumed iterator but rather indicate the
// consumer's position relative to other consumers. The purpose is to allow a
// consumer to know it should stop consuming items when another consumer finds a
// better match.

// An indexed consumer could specialize to use the real indexes instead, but we
// don't implement that for now. The only downside of the current approach is
// that in some cases, iterators very close to each other will have the same
// range and therefore not be able to stop processing if one of them finds a
// better match than the others.

#[derive(Copy, Clone)]
enum MatchPosition {
    Leftmost,
    Rightmost,
}

// Returns true if pos1 is a better match than pos2 according to MatchPosition
fn better_position(pos1: usize, pos2: usize, mp: MatchPosition) -> bool {
    match mp {
        MatchPosition::Leftmost => pos1 < pos2,
        MatchPosition::Rightmost => pos1 > pos2,
    }
}

pub fn find_first<PAR_ITER, FIND_OP>(pi: PAR_ITER, find_op: FIND_OP) -> Option<PAR_ITER::Item>
    where PAR_ITER: ParallelIterator,
          FIND_OP: Fn(&PAR_ITER::Item) -> bool + Sync
{
    let best_found = AtomicUsize::new(usize::max_value());
    let consumer = FindConsumer::new(&find_op, MatchPosition::Leftmost, &best_found);
    pi.drive_unindexed(consumer)
}

pub fn find_last<PAR_ITER, FIND_OP>(pi: PAR_ITER, find_op: FIND_OP) -> Option<PAR_ITER::Item>
    where PAR_ITER: ParallelIterator,
          FIND_OP: Fn(&PAR_ITER::Item) -> bool + Sync
{
    let best_found = AtomicUsize::new(0);
    let consumer = FindConsumer::new(&find_op, MatchPosition::Rightmost, &best_found);
    pi.drive_unindexed(consumer)
}

struct FindConsumer<'f, FIND_OP: 'f> {
    find_op: &'f FIND_OP,
    lower_bound: Cell<usize>,
    upper_bound: usize,
    match_position: MatchPosition,
    best_found: &'f AtomicUsize,
}

impl<'f, FIND_OP> FindConsumer<'f, FIND_OP> {
    fn new(find_op: &'f FIND_OP,
           match_position: MatchPosition,
           best_found: &'f AtomicUsize) -> Self {
        FindConsumer {
            find_op: find_op,
            lower_bound: Cell::new(0),
            upper_bound: usize::max_value(),
            match_position: match_position,
            best_found: best_found,
        }
    }

    fn current_index(&self) -> usize {
        match self.match_position {
            MatchPosition::Leftmost => self.lower_bound.get(),
            MatchPosition::Rightmost => self.upper_bound
        }
    }
}

impl<'f, ITEM, FIND_OP> Consumer<ITEM> for FindConsumer<'f, FIND_OP>
    where ITEM: Send,
          FIND_OP: Fn(&ITEM) -> bool + Sync
{
    type Folder = FindFolder<'f, ITEM, FIND_OP>;
    type Reducer = FindReducer;
    type Result = Option<ITEM>;

    fn cost(&mut self, cost: f64) -> f64 {
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, Self::Reducer) {
        let dir = self.match_position;
        (self.split_off(),
         self,
         FindReducer { match_position: dir })
    }

    fn into_folder(self) -> Self::Folder {
        FindFolder {
            find_op: self.find_op,
            boundary: self.current_index(),
            match_position: self.match_position,
            best_found: self.best_found,
            item: None,
        }
    }

    fn full(&self) -> bool {
        // can stop consuming if the best found index so far is *strictly*
        // better than anything this consumer will find
        better_position(self.best_found.load(Ordering::Relaxed),
                        self.current_index(),
                        self.match_position)
    }
}

impl<'f, ITEM, FIND_OP> UnindexedConsumer<ITEM> for FindConsumer<'f, FIND_OP>
    where ITEM: Send,
          FIND_OP: Fn(&ITEM) -> bool + Sync
{
    fn split_off(&self) -> Self {
        // Upper bound for one consumer will be lower bound for the other. This
        // overlap is okay, because only one of the bounds will be used for
        // comparing against best_found; the other is kept only to be able to
        // divide the range in half
        let old_lower_bound = self.lower_bound.get();
        let median = old_lower_bound + ((self.upper_bound - old_lower_bound) / 2);
        self.lower_bound.set(median);

        FindConsumer {
            find_op: self.find_op,
            lower_bound: Cell::new(old_lower_bound),
            upper_bound: median,
            match_position: self.match_position,
            best_found: self.best_found,
        }
    }

    fn to_reducer(&self) -> Self::Reducer {
        FindReducer { match_position: self.match_position }
    }
}

struct FindFolder<'f, ITEM, FIND_OP: 'f> {
    find_op: &'f FIND_OP,
    boundary: usize,
    match_position: MatchPosition,
    best_found: &'f AtomicUsize,
    item: Option<ITEM>,
}

impl<'f, FIND_OP: 'f + Fn(&ITEM) -> bool, ITEM> Folder<ITEM> for FindFolder<'f, ITEM, FIND_OP> {
    type Result = Option<ITEM>;

    fn consume(mut self, item: ITEM) -> Self {
        if (self.find_op)(&item) {
            // Continuously try to set best_found until we succeed or we
            // discover a better match was already found.
            let mut current = self.best_found.load(Ordering::Relaxed);
            let mut exchange_result = Result::Err(0);
            while better_position(self.boundary, current, self.match_position) &&
                  exchange_result.is_err() {
                exchange_result = self.best_found.compare_exchange_weak(current,
                                                                        self.boundary,
                                                                        Ordering::Relaxed,
                                                                        Ordering::Relaxed);
                if exchange_result.is_err() {
                    current = self.best_found.load(Ordering::Relaxed);
                }
            }

            if exchange_result.is_ok() {
                self.item = Some(item);
            }
        }
        self
    }

    fn complete(self) -> Self::Result {
        self.item
    }

    fn full(&self) -> bool {
        better_position(self.best_found.load(Ordering::Relaxed),
                        self.boundary,
                        self.match_position)
    }
}

struct FindReducer {
    match_position: MatchPosition
}

impl<ITEM> Reducer<Option<ITEM>> for FindReducer {
    fn reduce(self, left: Option<ITEM>, right: Option<ITEM>) -> Option<ITEM> {
        match self.match_position {
            MatchPosition::Leftmost => left.or(right),
            MatchPosition::Rightmost => right.or(left)
        }
    }
}
