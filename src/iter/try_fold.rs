use super::plumbing::*;
use super::*;

use super::private::Try;
use std::fmt::{self, Debug};
use std::marker::PhantomData;

impl<U, I, ID, F> TryFold<I, U, ID, F>
where
    I: ParallelIterator,
    F: Fn(U::Ok, I::Item) -> U + Sync + Send,
    ID: Fn() -> U::Ok + Sync + Send,
    U: Try + Send,
{
    pub(super) fn new(base: I, identity: ID, fold_op: F) -> Self {
        TryFold {
            base,
            identity,
            fold_op,
            marker: PhantomData,
        }
    }
}

/// `TryFold` is an iterator that applies a function over an iterator producing a single value.
/// This struct is created by the [`try_fold()`] method on [`ParallelIterator`]
///
/// [`try_fold()`]: trait.ParallelIterator.html#method.try_fold
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct TryFold<I, U, ID, F> {
    base: I,
    identity: ID,
    fold_op: F,
    marker: PhantomData<U>,
}

impl<U, I: ParallelIterator + Debug, ID, F> Debug for TryFold<I, U, ID, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryFold").field("base", &self.base).finish()
    }
}

impl<U, I, ID, F> ParallelIterator for TryFold<I, U, ID, F>
where
    I: ParallelIterator,
    F: Fn(U::Ok, I::Item) -> U + Sync + Send,
    ID: Fn() -> U::Ok + Sync + Send,
    U: Try + Send,
{
    type Item = U;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let consumer1 = TryFoldConsumer {
            base: consumer,
            identity: &self.identity,
            fold_op: &self.fold_op,
            marker: PhantomData,
        };
        self.base.drive_unindexed(consumer1)
    }
}

struct TryFoldConsumer<'c, U, C, ID: 'c, F: 'c> {
    base: C,
    identity: &'c ID,
    fold_op: &'c F,
    marker: PhantomData<U>,
}

impl<'r, U, T, C, ID, F> Consumer<T> for TryFoldConsumer<'r, U, C, ID, F>
where
    C: Consumer<U>,
    F: Fn(U::Ok, T) -> U + Sync,
    ID: Fn() -> U::Ok + Sync,
    U: Try + Send,
{
    type Folder = TryFoldFolder<'r, C::Folder, U, F>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (
            TryFoldConsumer { base: left, ..self },
            TryFoldConsumer {
                base: right,
                ..self
            },
            reducer,
        )
    }

    fn into_folder(self) -> Self::Folder {
        TryFoldFolder {
            base: self.base.into_folder(),
            result: Ok((self.identity)()),
            fold_op: self.fold_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'r, U, T, C, ID, F> UnindexedConsumer<T> for TryFoldConsumer<'r, U, C, ID, F>
where
    C: UnindexedConsumer<U>,
    F: Fn(U::Ok, T) -> U + Sync,
    ID: Fn() -> U::Ok + Sync,
    U: Try + Send,
{
    fn split_off_left(&self) -> Self {
        TryFoldConsumer {
            base: self.base.split_off_left(),
            ..*self
        }
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct TryFoldFolder<'r, C, U: Try, F: 'r> {
    base: C,
    fold_op: &'r F,
    result: Result<U::Ok, U::Error>,
}

impl<'r, C, U, F, T> Folder<T> for TryFoldFolder<'r, C, U, F>
where
    C: Folder<U>,
    F: Fn(U::Ok, T) -> U + Sync,
    U: Try,
{
    type Result = C::Result;

    fn consume(mut self, item: T) -> Self {
        let fold_op = self.fold_op;
        if let Ok(acc) = self.result {
            self.result = fold_op(acc, item).into_result();
        }
        self
    }

    fn complete(self) -> C::Result {
        let item = match self.result {
            Ok(ok) => U::from_ok(ok),
            Err(error) => U::from_error(error),
        };
        self.base.consume(item).complete()
    }

    fn full(&self) -> bool {
        self.result.is_err() || self.base.full()
    }
}

// ///////////////////////////////////////////////////////////////////////////

impl<U, I, F> TryFoldWith<I, U, F>
where
    I: ParallelIterator,
    F: Fn(U::Ok, I::Item) -> U + Sync,
    U: Try + Send,
    U::Ok: Clone + Send,
{
    pub(super) fn new(base: I, item: U::Ok, fold_op: F) -> Self {
        TryFoldWith {
            base,
            item,
            fold_op,
        }
    }
}

/// `TryFoldWith` is an iterator that applies a function over an iterator producing a single value.
/// This struct is created by the [`try_fold_with()`] method on [`ParallelIterator`]
///
/// [`try_fold_with()`]: trait.ParallelIterator.html#method.try_fold_with
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct TryFoldWith<I, U: Try, F> {
    base: I,
    item: U::Ok,
    fold_op: F,
}

impl<I: ParallelIterator + Debug, U: Try, F> Debug for TryFoldWith<I, U, F>
where
    U::Ok: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryFoldWith")
            .field("base", &self.base)
            .field("item", &self.item)
            .finish()
    }
}

impl<U, I, F> ParallelIterator for TryFoldWith<I, U, F>
where
    I: ParallelIterator,
    F: Fn(U::Ok, I::Item) -> U + Sync + Send,
    U: Try + Send,
    U::Ok: Clone + Send,
{
    type Item = U;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let consumer1 = TryFoldWithConsumer {
            base: consumer,
            item: self.item,
            fold_op: &self.fold_op,
        };
        self.base.drive_unindexed(consumer1)
    }
}

struct TryFoldWithConsumer<'c, C, U: Try, F: 'c> {
    base: C,
    item: U::Ok,
    fold_op: &'c F,
}

impl<'r, U, T, C, F> Consumer<T> for TryFoldWithConsumer<'r, C, U, F>
where
    C: Consumer<U>,
    F: Fn(U::Ok, T) -> U + Sync,
    U: Try + Send,
    U::Ok: Clone + Send,
{
    type Folder = TryFoldFolder<'r, C::Folder, U, F>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (
            TryFoldWithConsumer {
                base: left,
                item: self.item.clone(),
                ..self
            },
            TryFoldWithConsumer {
                base: right,
                ..self
            },
            reducer,
        )
    }

    fn into_folder(self) -> Self::Folder {
        TryFoldFolder {
            base: self.base.into_folder(),
            result: Ok(self.item),
            fold_op: self.fold_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'r, U, T, C, F> UnindexedConsumer<T> for TryFoldWithConsumer<'r, C, U, F>
where
    C: UnindexedConsumer<U>,
    F: Fn(U::Ok, T) -> U + Sync,
    U: Try + Send,
    U::Ok: Clone + Send,
{
    fn split_off_left(&self) -> Self {
        TryFoldWithConsumer {
            base: self.base.split_off_left(),
            item: self.item.clone(),
            ..*self
        }
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}
