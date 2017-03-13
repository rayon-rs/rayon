use super::ParallelIterator;
use super::len::*;
use super::internal::*;
use super::noop::*;

pub fn for_each<I, F, T>(pi: I, op: &F)
    where I: ParallelIterator<Item = T>,
          F: Fn(T) + Sync,
          T: Send
{
    let consumer = ForEachConsumer { op: op };
    pi.drive_unindexed(consumer)
}

struct ForEachConsumer<'f, F: 'f> {
    op: &'f F,
}

impl<'f, F, T> Consumer<T> for ForEachConsumer<'f, F>
    where F: Fn(T) + Sync
{
    type Folder = ForEachConsumer<'f, F>;
    type Reducer = NoopReducer;
    type Result = ();

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, NoopReducer) {
        (self.split_off_left(), self.split_off_left(), NoopReducer)
    }

    fn into_folder(self) -> Self {
        self
    }
}

impl<'f, F, T> Folder<T> for ForEachConsumer<'f, F>
    where F: Fn(T) + Sync
{
    type Result = ();

    fn consume(self, item: T) -> Self {
        (self.op)(item);
        self
    }

    fn complete(self) {}
}

impl<'f, F, T> UnindexedConsumer<T> for ForEachConsumer<'f, F>
    where F: Fn(T) + Sync
{
    fn split_off_left(&self) -> Self {
        ForEachConsumer { op: self.op }
    }

    fn to_reducer(&self) -> NoopReducer {
        NoopReducer
    }
}

