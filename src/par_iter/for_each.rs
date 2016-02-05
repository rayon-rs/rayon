use super::ParallelIterator;
use super::len::*;
use super::internal::*;
use super::util::*;

pub fn for_each<PAR_ITER,OP,T>(pi: PAR_ITER, op: &OP)
    where PAR_ITER: ParallelIterator<Item=T>,
          OP: Fn(T) + Sync,
          T: Send,
{
    let consumer: ForEachConsumer<OP, T> =
        ForEachConsumer { op: op, phantoms: PhantomType::new() };
    pi.drive_unindexed(consumer)
}

struct ForEachConsumer<'f, OP: 'f, ITEM> {
    op: &'f OP,
    phantoms: PhantomType<ITEM>
}

impl<'f, OP, ITEM> Consumer<'f> for ForEachConsumer<'f, OP, ITEM>
    where OP: Fn(ITEM) + Sync, ITEM: 'f,
{
    type Item = ITEM;
    type SeqState = ();
    type Result = ();

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, _index: usize) -> (Self, Self) {
        (self.split(), self.split())
    }

    unsafe fn start(&mut self) {
    }

    unsafe fn consume(&mut self, _prev_value: (), item: ITEM) {
        (self.op)(item);
    }

    unsafe fn complete(self, _state: ()) {
    }

    unsafe fn reduce(_: (), _: ()) {
    }
}

impl<'c, OP, ITEM> UnindexedConsumer<'c> for ForEachConsumer<'c, OP, ITEM>
    where OP: Fn(ITEM) + Sync + 'c, ITEM: 'c,
{
    fn split(&self) -> Self {
        ForEachConsumer { op: self.op, phantoms: PhantomType::new() }
    }
}
