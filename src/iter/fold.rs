use super::internal::*;
use super::len::*;
use super::*;

pub fn fold<U, BASE, ID, F>(base: BASE,
                                        identity: ID,
                                        fold_op: F)
                                        -> Fold<BASE, ID, F>
    where BASE: ParallelIterator,
          F: Fn(U, BASE::Item) -> U + Sync,
          ID: Fn() -> U + Sync,
          U: Send
{
    Fold {
        base: base,
        identity: identity,
        fold_op: fold_op,
    }
}

/// `Fold` is an iterator that applies a function over an iterator producing a single value.
/// This struct is created by the [`fold()`] method on [`ParallelIterator`]
///
/// [`fold()`]: trait.ParallelIterator.html#method.fold
/// [`ParallelIterator`]: trait.ParallelIterator.html
pub struct Fold<BASE, ID, F> {
    base: BASE,
    identity: ID,
    fold_op: F,
}

impl<U, BASE, ID, F> ParallelIterator for Fold<BASE, ID, F>
    where BASE: ParallelIterator,
          F: Fn(U, BASE::Item) -> U + Sync,
          ID: Fn() -> U + Sync,
          U: Send
{
    type Item = U;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = FoldConsumer {
            base: consumer,
            fold_op: &self.fold_op,
            identity: &self.identity,
        };
        self.base.drive_unindexed(consumer1)
    }
}

impl<U, BASE, ID, F> BoundedParallelIterator for Fold<BASE, ID, F>
    where BASE: BoundedParallelIterator,
          F: Fn(U, BASE::Item) -> U + Sync,
          ID: Fn() -> U + Sync,
          U: Send
{
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<'c, C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        let consumer1 = FoldConsumer {
            base: consumer,
            fold_op: &self.fold_op,
            identity: &self.identity,
        };
        self.base.drive(consumer1)
    }
}

struct FoldConsumer<'c, C, ID: 'c, F: 'c> {
    base: C,
    fold_op: &'c F,
    identity: &'c ID,
}

impl<'r, U, T, C, ID, F> Consumer<T> for FoldConsumer<'r, C, ID, F>
    where C: Consumer<U>,
          F: Fn(U, T) -> U + Sync,
          ID: Fn() -> U + Sync,
          U: Send
{
    type Folder = FoldFolder<'r, C::Folder, U, F>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (FoldConsumer { base: left, ..self }, FoldConsumer { base: right, ..self }, reducer)
    }

    fn into_folder(self) -> Self::Folder {
        FoldFolder {
            base: self.base.into_folder(),
            item: (self.identity)(),
            fold_op: self.fold_op,
        }
    }
}

impl<'r, U, ITEM, C, ID, F> UnindexedConsumer<ITEM>
    for FoldConsumer<'r, C, ID, F>
    where C: UnindexedConsumer<U>,
          F: Fn(U, ITEM) -> U + Sync,
          ID: Fn() -> U + Sync,
          U: Send
{
    fn split_off_left(&self) -> Self {
        FoldConsumer { base: self.base.split_off_left(), ..*self }
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

pub struct FoldFolder<'r, C, ID, F: 'r> {
    base: C,
    fold_op: &'r F,
    item: ID,
}

impl<'r, C, ID, F, ITEM> Folder<ITEM> for FoldFolder<'r, C, ID, F>
    where C: Folder<ID>,
          F: Fn(ID, ITEM) -> ID + Sync
{
    type Result = C::Result;

    fn consume(self, item: ITEM) -> Self {
        let item = (self.fold_op)(self.item, item);
        FoldFolder {
            base: self.base,
            fold_op: self.fold_op,
            item: item,
        }
    }

    fn complete(self) -> C::Result {
        self.base.consume(self.item).complete()
    }
}
