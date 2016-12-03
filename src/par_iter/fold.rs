use super::internal::*;
use super::len::*;
use super::*;

pub fn fold<U, BASE, IDENTITY, FOLD_OP>(base: BASE,
                                        identity: IDENTITY,
                                        fold_op: FOLD_OP)
                                        -> Fold<BASE, IDENTITY, FOLD_OP>
    where BASE: ParallelIterator,
          FOLD_OP: Fn(U, BASE::Item) -> U + Sync,
          IDENTITY: Fn() -> U + Sync,
          U: Send
{
    Fold {
        base: base,
        identity: identity,
        fold_op: fold_op,
    }
}

pub struct Fold<BASE, IDENTITY, FOLD_OP> {
    base: BASE,
    identity: IDENTITY,
    fold_op: FOLD_OP,
}

impl<U, BASE, IDENTITY, FOLD_OP> ParallelIterator for Fold<BASE, IDENTITY, FOLD_OP>
    where BASE: ParallelIterator,
          FOLD_OP: Fn(U, BASE::Item) -> U + Sync,
          IDENTITY: Fn() -> U + Sync,
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

impl<U, BASE, IDENTITY, FOLD_OP> BoundedParallelIterator for Fold<BASE, IDENTITY, FOLD_OP>
    where BASE: BoundedParallelIterator,
          FOLD_OP: Fn(U, BASE::Item) -> U + Sync,
          IDENTITY: Fn() -> U + Sync,
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

pub struct FoldConsumer<'c, C, IDENTITY: 'c, FOLD_OP: 'c> {
    base: C,
    fold_op: &'c FOLD_OP,
    identity: &'c IDENTITY,
}

impl<'r, U, T, C, IDENTITY, FOLD_OP> Consumer<T> for FoldConsumer<'r, C, IDENTITY, FOLD_OP>
    where C: Consumer<U>,
          FOLD_OP: Fn(U, T) -> U + Sync,
          IDENTITY: Fn() -> U + Sync,
          U: Send
{
    type Folder = FoldFolder<'r, C::Folder, U, FOLD_OP>;
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

impl<'r, U, ITEM, C, IDENTITY, FOLD_OP> UnindexedConsumer<ITEM>
    for FoldConsumer<'r, C, IDENTITY, FOLD_OP>
    where C: UnindexedConsumer<U>,
          FOLD_OP: Fn(U, ITEM) -> U + Sync,
          IDENTITY: Fn() -> U + Sync,
          U: Send
{
    fn split_off(&self) -> Self {
        FoldConsumer { base: self.base.split_off(), ..*self }
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

pub struct FoldFolder<'r, C, IDENTITY, FOLD_OP: 'r> {
    base: C,
    fold_op: &'r FOLD_OP,
    item: IDENTITY,
}

impl<'r, C, IDENTITY, FOLD_OP, ITEM> Folder<ITEM> for FoldFolder<'r, C, IDENTITY, FOLD_OP>
    where C: Folder<IDENTITY>,
          FOLD_OP: Fn(IDENTITY, ITEM) -> IDENTITY + Sync
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
