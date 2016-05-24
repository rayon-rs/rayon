use super::ParallelIterator;
use super::internal::*;

pub fn for_each<PAR_ITER,OP,T>(pi: PAR_ITER, op: &OP)
    where PAR_ITER: ParallelIterator<Item=T>,
          OP: Fn(T) + Sync,
          T: Send,
{
    let consumer = ForEachConsumer { op: op };
    pi.drive_unindexed(consumer)
}

struct ForEachConsumer<'f, OP: 'f> {
    op: &'f OP,
}

impl<'f, OP, ITEM> Consumer<ITEM> for ForEachConsumer<'f, OP>
    where OP: Fn(ITEM) + Sync,
{
    type Folder = ForEachConsumer<'f, OP>;
    type Reducer = NoopReducer;
    type Result = ();

    fn sequential_threshold(&self) -> usize {
        1
    }

    fn split_at(self, _index: usize) -> (Self, Self, NoopReducer) {
        (self.split_off(), self.split_off(), NoopReducer)
    }

    fn into_folder(self) -> Self {
        self
    }
}

impl<'f, OP, ITEM> Folder<ITEM> for ForEachConsumer<'f, OP>
    where OP: Fn(ITEM) + Sync,
{
    type Result = ();

    fn consume(self, item: ITEM) -> Self {
        (self.op)(item);
        self
    }

    fn complete(self) {
    }
}

impl<'f, OP, ITEM> UnindexedConsumer<ITEM> for ForEachConsumer<'f, OP>
    where OP: Fn(ITEM) + Sync,
{
    fn split_off(&self) -> Self {
        ForEachConsumer { op: self.op }
    }

    fn to_reducer(&self) -> NoopReducer {
        NoopReducer
    }
}
