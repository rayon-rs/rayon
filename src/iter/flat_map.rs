use super::plumbing::*;
use super::*;

use std::fmt::{self, Debug};

/// `FlatMap` maps each element to a parallel iterator, then flattens these iterators together.
/// This struct is created by the [`flat_map()`] method on [`ParallelIterator`]
///
/// [`flat_map()`]: ParallelIterator::flat_map()
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct FlatMap<I, F> {
    base: I,
    map_op: F,
}

impl<I: Debug, F> Debug for FlatMap<I, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatMap").field("base", &self.base).finish()
    }
}

impl<I, F> FlatMap<I, F> {
    /// Creates a new `FlatMap` iterator.
    pub(super) fn new(base: I, map_op: F) -> Self {
        FlatMap { base, map_op }
    }
}

impl<I, F, PI> ParallelIterator for FlatMap<I, F>
where
    I: ParallelIterator,
    F: Fn(I::Item) -> PI + Sync + Send,
    PI: IntoParallelIterator,
{
    type Item = PI::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let consumer = FlatMapConsumer::new(consumer, &self.map_op);
        self.base.drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        let base_len = self.base.opt_len()?;
        let sub_len = PI::const_length()?;

        base_len.checked_mul(sub_len)
    }

    fn const_length() -> Option<usize> {
        let base_len = I::const_length()?;
        let sub_len = PI::const_length()?;

        base_len.checked_mul(sub_len)
    }
}

// ////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct FlatMapConsumer<'f, C, F> {
    base: C,
    map_op: &'f F,
}

impl<'f, C, F> FlatMapConsumer<'f, C, F> {
    fn new(base: C, map_op: &'f F) -> Self {
        FlatMapConsumer { base, map_op }
    }
}

impl<'f, T, U, C, F> Consumer<T> for FlatMapConsumer<'f, C, F>
where
    C: UnindexedConsumer<U::Item>,
    F: Fn(T) -> U + Sync,
    U: IntoParallelIterator,
{
    type Folder = FlatMapFolder<'f, C, F, C::Result>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, C::Reducer) {
        // FIXME: I have NO Idea if this is correct, it almost definately is NOT
        //        But the tests pass, and I no longer panic so that is a start
        let (left, right, reducer) = if let Some(inner_len) = U::const_length() {
            self.base.split_at(index * inner_len)
        } else {
            self.base.split_at(index)
        };
        
        (
            FlatMapConsumer::new(left, self.map_op),
            FlatMapConsumer::new(right, self.map_op),
            reducer,
        )
    }

    fn into_folder(self) -> Self::Folder {
        FlatMapFolder {
            base: self.base,
            map_op: self.map_op,
            previous: None,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'f, T, U, C, F> UnindexedConsumer<T> for FlatMapConsumer<'f, C, F>
where
    C: UnindexedConsumer<U::Item>,
    F: Fn(T) -> U + Sync,
    U: IntoParallelIterator,
{
    fn split_off_left(&self) -> Self {
        FlatMapConsumer::new(self.base.split_off_left(), self.map_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct FlatMapFolder<'f, C, F, R> {
    base: C,
    map_op: &'f F,
    previous: Option<R>,
}

impl<'f, T, U, C, F> Folder<T> for FlatMapFolder<'f, C, F, C::Result>
where
    C: UnindexedConsumer<U::Item>,
    F: Fn(T) -> U + Sync,
    U: IntoParallelIterator,
{
    type Result = C::Result;

    fn consume(self, item: T) -> Self {
        let map_op = self.map_op;
        let par_iter = map_op(item).into_par_iter();
        // TODO: This is a hack, to fake specialisation until it is in Rust proper
        let (consumer, rest, reducer) = if let Some(inner_len) = U::const_length() {
            // We can to use split_at
            self.base.split_at(inner_len)
        } else {
            // Use the normal Unindexed version
            let reducer = self.base.to_reducer();
            (self.base.split_off_left(), self.base, reducer)
        };

        let result = par_iter.drive_unindexed(consumer);

        let previous = match self.previous {
            None => Some(result),
            Some(previous) => {
                Some(reducer.reduce(previous, result))
            }
        };

        FlatMapFolder {
            base: rest,
            map_op,
            previous,
        }
    }

    fn complete(self) -> Self::Result {
        match self.previous {
            Some(previous) => previous,
            None => self.base.into_folder().complete(),
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
