use self::flat_combine::FlatCombiner;

use super::ParallelIterator;
use super::len::*;
use super::internal::*;

use thread_pool::in_worker;

mod flat_combine;

pub fn for_each_atomic<PAR_ITER,OP,T>(pi: PAR_ITER, op: &mut OP)
    where PAR_ITER: ParallelIterator<Item=T> + Send,
          OP: FnMut(T) + Send,
          T: Send,
{
    in_worker(|_| {
        unsafe { // we assert we are in worker thread
            let combiner = FlatCombiner::new(op);
            let consumer = ForEachLockedConsumer { combiner: &combiner };
            pi.drive_unindexed(consumer)
        }
    })
}

struct ForEachLockedConsumer<'f, OP: 'f, ITEM: 'f>
    where ITEM: Send,
          OP: FnMut(ITEM) + Send,
{
    combiner: &'f FlatCombiner<&'f mut OP, ITEM>,
}

impl<'f, OP, ITEM> Consumer<ITEM> for ForEachLockedConsumer<'f, OP, ITEM>
    where OP: FnMut(ITEM) + Send, ITEM: Send,
{
    type Folder = ForEachLockedConsumer<'f, OP, ITEM>;
    type Reducer = NoopReducer;
    type Result = ();

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, NoopReducer) {
        (self.split_off(), self.split_off(), NoopReducer)
    }

    fn into_folder(self) -> Self {
        self
    }
}

impl<'f, OP, ITEM> Folder<ITEM> for ForEachLockedConsumer<'f, OP, ITEM>
    where OP: FnMut(ITEM) + Send, ITEM: Send,
{
    type Result = ();

    fn consume(self, item: ITEM) -> Self {
        unsafe { // we assert we are in worker thread in appropriate registry
            self.combiner.produce(item);
        }
        self
    }

    fn complete(self) {
    }
}

impl<'f, OP, ITEM> UnindexedConsumer<ITEM> for ForEachLockedConsumer<'f, OP, ITEM>
    where OP: FnMut(ITEM) + Send, ITEM: Send,
{
    fn split_off(&self) -> Self {
        ForEachLockedConsumer { combiner: self.combiner }
    }

    fn to_reducer(&self) -> NoopReducer {
        NoopReducer
    }
}
