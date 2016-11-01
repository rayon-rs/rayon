use super::ParallelIterator;
use super::len::*;
use super::internal::*;

pub fn fold<PAR_ITER,I,FOLD_OP,REDUCE_OP>(pi: PAR_ITER,
                                          identity: &I,
                                          fold_op: &FOLD_OP,
                                          reduce_op: &REDUCE_OP)
                                          -> I
    where PAR_ITER: ParallelIterator,
          FOLD_OP: Fn(I, PAR_ITER::Item) -> I + Sync,
          REDUCE_OP: Fn(I, I) -> I + Sync,
          I: Clone + Send + Sync,
{
    let consumer = FoldConsumer { identity: identity,
                                  fold_op: fold_op,
                                  reduce_op: reduce_op };
    pi.drive_unindexed(consumer)
}

struct FoldConsumer<'r, I:'r, FOLD_OP: 'r, REDUCE_OP: 'r> {
    identity: &'r I,
    fold_op: &'r FOLD_OP,
    reduce_op: &'r REDUCE_OP,
}

impl<'r, I, FOLD_OP, REDUCE_OP> Copy for FoldConsumer<'r, I, FOLD_OP, REDUCE_OP> {
}

impl<'r, I, FOLD_OP, REDUCE_OP> Clone for FoldConsumer<'r, I, FOLD_OP, REDUCE_OP> {
    fn clone(&self) -> Self { *self }
}

impl<'r, ITEM, I, FOLD_OP, REDUCE_OP> Consumer<ITEM>
    for FoldConsumer<'r, I, FOLD_OP, REDUCE_OP>
    where FOLD_OP: Fn(I, ITEM) -> I + Sync,
          REDUCE_OP: Fn(I, I) -> I + Sync,
          I: Clone + Send + Sync,
{
    type Folder = FoldFolder<'r, I, FOLD_OP>;
    type Reducer = Self;
    type Result = I;

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, Self) {
        (self, self, self)
    }

    fn into_folder(self) -> Self::Folder {
        FoldFolder { item: self.identity.clone(),
                     fold_op: self.fold_op }
    }

    fn should_continue(&self) -> bool {
        true
    }
}

impl<'r, ITEM, I, FOLD_OP, REDUCE_OP> UnindexedConsumer<ITEM>
    for FoldConsumer<'r, I, FOLD_OP, REDUCE_OP>
    where FOLD_OP: Fn(I, ITEM) -> I + Sync,
          REDUCE_OP: Fn(I, I) -> I + Sync,
          I: Clone + Send + Sync,
{
    fn split_off(&self) -> Self {
        *self
    }

    fn to_reducer(&self) -> Self::Reducer {
        *self
    }
}

impl<'r, I, FOLD_OP, REDUCE_OP> Reducer<I>
    for FoldConsumer<'r, I, FOLD_OP, REDUCE_OP>
    where REDUCE_OP: Fn(I, I) -> I + Sync,
          I: Clone + Send + Sync,
{
    fn reduce(self, left: I, right: I) -> I {
        (self.reduce_op)(left, right)
    }
}

pub struct FoldFolder<'r, I, FOLD_OP: 'r> {
    fold_op: &'r FOLD_OP,
    item: I,
}

impl<'r, I, FOLD_OP, ITEM> Folder<ITEM>
    for FoldFolder<'r, I, FOLD_OP>
    where FOLD_OP: Fn(I, ITEM) -> I + Sync,
{
    type Result = I;

    fn consume(self, item: ITEM) -> Self {
        let item = (self.fold_op)(self.item, item);
        FoldFolder { fold_op: self.fold_op, item: item }
    }

    fn complete(self) -> I {
        self.item
    }

    fn should_continue(&self) -> bool {
        true
    }
}

