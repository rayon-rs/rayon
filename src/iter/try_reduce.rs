use super::plumbing::*;
use super::ParallelIterator;

use super::private::Try;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn try_reduce<PI, R, ID, T>(pi: PI, identity: ID, reduce_op: R) -> T
where
    PI: ParallelIterator<Item = T>,
    R: Fn(T::Ok, T::Ok) -> T + Sync,
    ID: Fn() -> T::Ok + Sync,
    T: Try + Send,
{
    let full = AtomicBool::new(false);
    let consumer = TryReduceConsumer {
        identity: &identity,
        reduce_op: &reduce_op,
        full: &full,
    };
    pi.drive_unindexed(consumer)
}

struct TryReduceConsumer<'r, R: 'r, ID: 'r> {
    identity: &'r ID,
    reduce_op: &'r R,
    full: &'r AtomicBool,
}

impl<'r, R, ID> Copy for TryReduceConsumer<'r, R, ID> {}

impl<'r, R, ID> Clone for TryReduceConsumer<'r, R, ID> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'r, R, ID, T> Consumer<T> for TryReduceConsumer<'r, R, ID>
where
    R: Fn(T::Ok, T::Ok) -> T + Sync,
    ID: Fn() -> T::Ok + Sync,
    T: Try + Send,
{
    type Folder = TryReduceFolder<'r, R, T>;
    type Reducer = Self;
    type Result = T;

    fn split_at(self, _index: usize) -> (Self, Self, Self) {
        (self, self, self)
    }

    fn into_folder(self) -> Self::Folder {
        TryReduceFolder {
            reduce_op: self.reduce_op,
            result: Ok((self.identity)()),
            full: self.full,
        }
    }

    fn full(&self) -> bool {
        self.full.load(Ordering::Relaxed)
    }
}

impl<'r, R, ID, T> UnindexedConsumer<T> for TryReduceConsumer<'r, R, ID>
where
    R: Fn(T::Ok, T::Ok) -> T + Sync,
    ID: Fn() -> T::Ok + Sync,
    T: Try + Send,
{
    fn split_off_left(&self) -> Self {
        *self
    }

    fn to_reducer(&self) -> Self::Reducer {
        *self
    }
}

impl<'r, R, ID, T> Reducer<T> for TryReduceConsumer<'r, R, ID>
where
    R: Fn(T::Ok, T::Ok) -> T + Sync,
    T: Try,
{
    fn reduce(self, left: T, right: T) -> T {
        match (left.into_result(), right.into_result()) {
            (Ok(left), Ok(right)) => (self.reduce_op)(left, right),
            (Err(e), _) | (_, Err(e)) => T::from_error(e),
        }
    }
}

struct TryReduceFolder<'r, R: 'r, T: Try> {
    reduce_op: &'r R,
    result: Result<T::Ok, T::Error>,
    full: &'r AtomicBool,
}

impl<'r, R, T> Folder<T> for TryReduceFolder<'r, R, T>
where
    R: Fn(T::Ok, T::Ok) -> T,
    T: Try,
{
    type Result = T;

    fn consume(mut self, item: T) -> Self {
        let reduce_op = self.reduce_op;
        if let Ok(left) = self.result {
            self.result = match item.into_result() {
                Ok(right) => reduce_op(left, right).into_result(),
                Err(error) => Err(error),
            };
        }
        if self.result.is_err() {
            self.full.store(true, Ordering::Relaxed)
        }
        self
    }

    fn complete(self) -> T {
        match self.result {
            Ok(ok) => T::from_ok(ok),
            Err(error) => T::from_error(error),
        }
    }

    fn full(&self) -> bool {
        self.full.load(Ordering::Relaxed)
    }
}
