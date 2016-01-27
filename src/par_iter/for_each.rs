use super::ParallelIterator;
use super::len::*;
use super::internal::*;
use super::util::*;

pub fn for_each<PAR_ITER,OP,T>(pi: PAR_ITER, op: &OP)
    where PAR_ITER: ParallelIterator<Item=T>,
          OP: Fn(T) + Sync,
          T: Send,
{
    let consumer: ForEachConsumer<PAR_ITER::Item, OP> = ForEachConsumer { data: PhantomType::new() };
    pi.drive_unindexed(consumer, op)
}

struct ForEachConsumer<ITEM, OP>
    where OP: Fn(ITEM) + Sync,
{
    data: PhantomType<(ITEM, OP)>
}

impl<'c, ITEM, OP> Consumer<'c> for ForEachConsumer<ITEM, OP>
    where OP: Fn(ITEM) + Sync + 'c, ITEM: 'c,
{
    type Item = ITEM;
    type Shared = OP;
    type SeqState = ();
    type Result = ();

    fn cost(&mut self, _shared: &Self::Shared, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, _: &Self::Shared, _index: usize) -> (Self, Self) {
        (self.split(), self.split())
    }

    unsafe fn start(&mut self, _reduce_op: &OP) {
    }

    unsafe fn consume(&mut self,
                      op: &OP,
                      _prev_value: (),
                      item: ITEM) {
        op(item);
    }

    unsafe fn complete(self,
                       _op: &OP,
                       _state: ()) {
    }

    unsafe fn reduce(_reduce_op: &OP, _: (), _: ()) {
    }
}

impl<'c, ITEM, OP> UnindexedConsumer<'c> for ForEachConsumer<ITEM, OP>
    where OP: Fn(ITEM) + Sync + 'c, ITEM: 'c,
{
    fn split(&self) -> Self {
        ForEachConsumer { data: PhantomType::new() }
    }
}
