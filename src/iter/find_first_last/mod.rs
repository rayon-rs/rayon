use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use super::internal::*;
use super::*;

#[cfg(test)]
mod test;

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

pub fn find_first<I, P>(pi: I, find_op: P) -> Option<I::Item>
    where I: ParallelIterator,
          P: Fn(&I::Item) -> bool + Sync
{
    let best_found = AtomicUsize::new(usize::max_value());
    let consumer = FindConsumer::new(&find_op, MatchPosition::Leftmost, &best_found);
    pi.drive_unindexed(consumer)
}

pub fn find_last<I, P>(pi: I, find_op: P) -> Option<I::Item>
    where I: ParallelIterator,
          P: Fn(&I::Item) -> bool + Sync
{
    let best_found = AtomicUsize::new(0);
    let consumer = FindConsumer::new(&find_op, MatchPosition::Rightmost, &best_found);
    pi.drive_unindexed(consumer)
}

struct FindConsumer<'p, P: 'p> {
    find_op: &'p P,
    lower_bound: Cell<usize>,
    upper_bound: usize,
    match_position: MatchPosition,
    best_found: &'p AtomicUsize,
}

impl<'p, P> FindConsumer<'p, P> {
    fn new(find_op: &'p P, match_position: MatchPosition, best_found: &'p AtomicUsize) -> Self {
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
            MatchPosition::Rightmost => self.upper_bound,
        }
    }
}

impl<'p, T, P> Consumer<T> for FindConsumer<'p, P>
    where T: Send,
          P: Fn(&T) -> bool + Sync
{
    type Folder = FindFolder<'p, T, P>;
    type Reducer = FindReducer;
    type Result = Option<T>;

    fn split_at(self, _index: usize) -> (Self, Self, Self::Reducer) {
        let dir = self.match_position;
        (self.split_off_left(), self, FindReducer { match_position: dir })
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

impl<'p, T, P> UnindexedConsumer<T> for FindConsumer<'p, P>
    where T: Send,
          P: Fn(&T) -> bool + Sync
{
    fn split_off_left(&self) -> Self {
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

struct FindFolder<'p, T, P: 'p> {
    find_op: &'p P,
    boundary: usize,
    match_position: MatchPosition,
    best_found: &'p AtomicUsize,
    item: Option<T>,
}

impl<'p, P: 'p + Fn(&T) -> bool, T> Folder<T> for FindFolder<'p, T, P> {
    type Result = Option<T>;

    fn consume(mut self, item: T) -> Self {
        let found_best_in_range = match self.match_position {
            MatchPosition::Leftmost => self.item.is_some(),
            MatchPosition::Rightmost => false,
        };

        if !found_best_in_range && (self.find_op)(&item) {
            // Continuously try to set best_found until we succeed or we
            // discover a better match was already found.
            let mut current = self.best_found.load(Ordering::Relaxed);
            loop {
                if better_position(current, self.boundary, self.match_position) {
                    break;
                }
                match self.best_found.compare_exchange_weak(current,
                                                            self.boundary,
                                                            Ordering::Relaxed,
                                                            Ordering::Relaxed) {
                    Ok(_) => {
                        self.item = Some(item);
                        break;
                    }
                    Err(v) => current = v,
                }
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
        find_op: &(|&_: &i32| -> bool { true }),
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
        find_op: &(|&_: &i32| -> bool { true }),
        boundary: 0,
        match_position: MatchPosition::Rightmost,
        best_found: &best_found,
        item: None,
    };
    let f = f.consume(0_i32).consume(1_i32).consume(2_i32);
    assert_eq!(f.complete(), Some(2_i32));
}

struct FindReducer {
    match_position: MatchPosition,
}

impl<T> Reducer<Option<T>> for FindReducer {
    fn reduce(self, left: Option<T>, right: Option<T>) -> Option<T> {
        match self.match_position {
            MatchPosition::Leftmost => left.or(right),
            MatchPosition::Rightmost => right.or(left),
        }
    }
}
