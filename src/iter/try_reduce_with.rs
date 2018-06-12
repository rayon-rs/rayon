use super::ParallelIterator;
use super::plumbing::*;

use super::private::Try;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn try_reduce_with<PI, R, T>(pi: PI, reduce_op: R) -> Option<T>
    where PI: ParallelIterator<Item = T>,
          R: Fn(T::Ok, T::Ok) -> T + Sync,
          T: Try + Send
{
    let full = AtomicBool::new(false);
    let consumer = TryReduceWithConsumer {
        reduce_op: &reduce_op,
        full: &full,
    };
    pi.drive_unindexed(consumer)
}

struct TryReduceWithConsumer<'r, R: 'r> {
    reduce_op: &'r R,
    full: &'r AtomicBool,
}

impl<'r, R> Copy for TryReduceWithConsumer<'r, R> {}

impl<'r, R> Clone for TryReduceWithConsumer<'r, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'r, R, T> Consumer<T> for TryReduceWithConsumer<'r, R>
    where R: Fn(T::Ok, T::Ok) -> T + Sync,
          T: Try + Send
{
    type Folder = TryReduceWithFolder<'r, R, T>;
    type Reducer = Self;
    type Result = Option<T>;

    fn split_at(self, _index: usize) -> (Self, Self, Self) {
        (self, self, self)
    }

    fn into_folder(self) -> Self::Folder {
        TryReduceWithFolder {
            reduce_op: self.reduce_op,
            opt_result: None,
            full: self.full,
        }
    }

    fn full(&self) -> bool {
        self.full.load(Ordering::Relaxed)
    }
}

impl<'r, R, T> UnindexedConsumer<T> for TryReduceWithConsumer<'r, R>
    where R: Fn(T::Ok, T::Ok) -> T + Sync,
          T: Try + Send
{
    fn split_off_left(&self) -> Self {
        *self
    }

    fn to_reducer(&self) -> Self::Reducer {
        *self
    }
}

impl<'r, R, T> Reducer<Option<T>> for TryReduceWithConsumer<'r, R>
    where R: Fn(T::Ok, T::Ok) -> T + Sync,
          T: Try
{
    fn reduce(self, left: Option<T>, right: Option<T>) -> Option<T> {
        let reduce_op = self.reduce_op;
        match (left, right) {
            (None, x) | (x, None) => x,
            (Some(a), Some(b)) => {
                match (a.into_result(), b.into_result()) {
                    (Ok(a), Ok(b)) => Some(reduce_op(a, b)),
                    (Err(e), _) | (_, Err(e)) => Some(T::from_error(e)),
                }
            }
        }
    }
}

struct TryReduceWithFolder<'r, R: 'r, T: Try> {
    reduce_op: &'r R,
    opt_result: Option<Result<T::Ok, T::Error>>,
    full: &'r AtomicBool,
}

impl<'r, R, T> Folder<T> for TryReduceWithFolder<'r, R, T>
    where R: Fn(T::Ok, T::Ok) -> T,
          T: Try
{
    type Result = Option<T>;

    fn consume(self, item: T) -> Self {
        let reduce_op = self.reduce_op;
        let result = match self.opt_result {
            None => item.into_result(),
            Some(Ok(a)) => match item.into_result() {
                Ok(b) => reduce_op(a, b).into_result(),
                Err(e) => Err(e),
            },
            Some(Err(e)) => Err(e),
        };
        if result.is_err() {
            self.full.store(true, Ordering::Relaxed)
        }
        TryReduceWithFolder {
            opt_result: Some(result),
            ..self
        }
    }

    fn complete(self) -> Option<T> {
        self.opt_result.map(|result| match result {
            Ok(ok) => T::from_ok(ok),
            Err(error) => T::from_error(error),
        })
    }

    fn full(&self) -> bool {
        self.full.load(Ordering::Relaxed)
    }
}
