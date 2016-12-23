use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use super::internal::*;
use super::*;
use super::len::*;

// The key optimization for find_first is that a consumer can stop its search if
// some consumer to its left already found a match (and similarly for consumers
// to the right for find_last). To make this work, all consumers need some
// notion of their position in the data relative to other consumers, including
// unindexed consumers that have no built-in notion of position.
//
// To solve this, we assign each consumer a lower and upper bound for an
// imaginary "range" of data that it consumes. The initial consumer starts with
// the range 0..usize::max_value(). The split divides this range in half so that
// one resulting consumer has the range 0..(usize::max_value() / 2), and the
// other has (usize::max_value() / 2)..usize::max_value(). Every subsequent
// split divides the range in half again until it cannot be split anymore
// (i.e. its length is 1), in which case the split returns two consumers with
// the same range. In that case both consumers will continue to consume all
// their data regardless of whether a better match is found, but the reducer
// will still return the correct answer.

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

#[test]
fn same_range_consumers_return_correct_answer() {
    let find_op = |x: &i32| x % 2 == 0;

    let first_found = AtomicUsize::new(usize::max_value());
    let first_consumer = FindConsumer::new(&find_op,
                                           MatchPosition::Leftmost,
                                           &first_found);

    // split until we have an indivisible range
    let bits_in_usize = usize::min_value().count_zeros();
    for i in 0..bits_in_usize {
        first_consumer.split_off();
    }

    let first_reducer = first_consumer.to_reducer();
    // the left and right folders should now have the same range, having
    // exhausted the resolution of usize
    let left_first_folder = first_consumer.split_off().into_folder();
    let right_first_folder = first_consumer.into_folder();

    let left_first_folder = left_first_folder.consume(0).consume(1);
    assert_eq!(left_first_folder.boundary, right_first_folder.boundary);
    // expect not full even though a better match has been found because the
    // ranges are the same
    assert!(!right_first_folder.full());
    let right_first_folder = right_first_folder.consume(2).consume(3);
    assert_eq!(first_reducer.reduce(left_first_folder.complete(),
                                    right_first_folder.complete()),
               Some(0));

    // same test, but for find_last
    let last_found = AtomicUsize::new(0);
    let last_consumer = FindConsumer::new(&find_op,
                                          MatchPosition::Rightmost,
                                          &last_found);
    for i in 0..bits_in_usize {
        last_consumer.split_off();
    }

    let last_reducer = last_consumer.to_reducer();
    // due to the exact calculation in split_off, the very last consumer has a
    // range of width 2, so we use the second-to-last consumer instead to get
    // the same boundary on both folders
    let last_consumer = last_consumer.split_off();
    let left_last_folder = last_consumer.split_off().into_folder();
    let right_last_folder = last_consumer.into_folder();
    let right_last_folder = right_last_folder.consume(2).consume(3);
    assert_eq!(left_last_folder.boundary, right_last_folder.boundary);
    // expect not full even though a better match has been found because the
    // ranges are the same
    assert!(!left_last_folder.full());
    let left_last_folder = left_last_folder.consume(0).consume(1);
    assert_eq!(last_reducer.reduce(left_last_folder.complete(),
                                   right_last_folder.complete()),
               Some(2));
}

impl<'f, ITEM, FIND_OP> UnindexedConsumer<ITEM> for FindConsumer<'f, FIND_OP>
    where ITEM: Send,
          FIND_OP: Fn(&ITEM) -> bool + Sync
{
    fn split_off(&self) -> Self {
        // Upper bound for one consumer will be lower bound for the other. This
        // overlap is okay, because only one of the bounds will be used for
        // comparing against best_found; the other is kept only to be able to
        // divide the range in half.
        //
        // When the resolution of usize has been exhausted (i.e. when
        // upper_bound = lower_bound), both results of this split will have the
        // same range. When that happens, we lose the ability to tell one
        // consumer to stop working when the other finds a better match, but the
        // reducer ensures that the best answer is still returned (see the test
        // above).
        //
        // This code assumes that the caller of split_off will use the result as
        // the *left* side of this iterator, and the remainder of self as the
        // *right* side.
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
        let found_best_in_range = match self.match_position {
            MatchPosition::Leftmost => self.item.is_some(),
            MatchPosition::Rightmost => false,
        };

        if !found_best_in_range && (self.find_op)(&item) {
            // Continuously try to set best_found until we succeed or we
            // discover a better match was already found.
            let mut current = self.best_found.load(Ordering::Relaxed);
            let mut exchange_result = Result::Err(0);
            while !better_position(current, self.boundary, self.match_position) &&
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
        let found_best_in_range = match self.match_position {
            MatchPosition::Leftmost => self.item.is_some(),
            MatchPosition::Rightmost => false,
        };

        found_best_in_range ||
            better_position(self.best_found.load(Ordering::Relaxed),
                            self.boundary,
                            self.match_position)
    }
}

// These tests requires that a folder be assigned to an iterator with more than
// one element. We can't necessarily determine when that will happen for a given
// input to find_first/find_last, so we test the folder directly here instead.
#[test]
fn find_first_folder_does_not_clobber_first_found() {
    let best_found = AtomicUsize::new(usize::max_value());
    let f = FindFolder {
        find_op: &(|&x: &i32| -> bool { true }),
        boundary: 0,
        match_position: MatchPosition::Leftmost,
        best_found: &best_found,
        item: None,
    };
    let f = f.consume(0_i32).consume(1_i32).consume(2_i32);
    assert!(f.full());
    assert_eq!(f.complete(), Some(0_i32));
}

#[test]
fn find_last_folder_yields_last_match() {
    let best_found = AtomicUsize::new(0);
    let f = FindFolder {
        find_op: &(|&x: &i32| -> bool { true }),
        boundary: 0,
        match_position: MatchPosition::Rightmost,
        best_found: &best_found,
        item: None,
    };
    let f = f.consume(0_i32).consume(1_i32).consume(2_i32);
    assert_eq!(f.complete(), Some(2_i32));
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
