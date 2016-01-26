use api::join;
use super::ParallelIterator;
use super::len::*;
use super::state::*;
use super::util::*;

pub fn for_each<PAR_ITER,OP,T>(pi: PAR_ITER, op: &OP)
    where PAR_ITER: ParallelIterator<Item=T>,
          PAR_ITER::State: Send,
          OP: Fn(T) + Sync,
          T: Send,
{
    let consumer: ForEachConsumer<PAR_ITER::Item, OP> = ForEachConsumer { data: PhantomType::new() };
    pi.drive(consumer, op)
}

fn for_each_helper<STATE,OP,T>(mut state: STATE,
                               shared: &STATE::Shared,
                               len: ParallelLen,
                               op: &OP)
    where STATE: ParallelIteratorState<Item=T> + Send,
          OP: Fn(T) + Sync,
          T: Send,
{
    if len.cost > THRESHOLD && len.maximal_len > 1 {
        let mid = len.maximal_len / 2;
        let (left, right) = state.split_at(mid);
        join(|| for_each_helper(left, shared, len.left_cost(mid), op),
             || for_each_helper(right, shared, len.right_cost(mid), op));
    } else {
        while let Some(item) = state.next(shared) {
            op(item);
        }
    }
}

struct ForEachConsumer<ITEM, OP>
    where OP: Fn(ITEM) + Sync,
{
    data: PhantomType<(ITEM, OP)>
}

impl<ITEM, OP> Copy for ForEachConsumer<ITEM, OP>
    where OP: Fn(ITEM) + Sync,
{
}

impl<ITEM, OP> Clone for ForEachConsumer<ITEM, OP>
    where OP: Fn(ITEM) + Sync,
{
    fn clone(&self) -> Self { *self }
}

impl<'c, ITEM, OP> Consumer<'c> for ForEachConsumer<ITEM, OP>
    where OP: Fn(ITEM) + Sync + 'c, ITEM: 'c,
{
    type Item = ITEM;
    type Shared = OP;
    type SeqState = ();
    type Result = ();

    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, _: &Self::Shared, _index: usize) -> (Self, Self) {
        (self, self)
    }

    unsafe fn start(&mut self, _reduce_op: &OP) {
    }

    unsafe fn consume(&mut self,
                      op: &OP,
                      prev_value: (),
                      item: ITEM) {
        op(item);
    }

    unsafe fn complete(self,
                       _op: &OP,
                       state: ()) {
    }

    unsafe fn reduce(reduce_op: &OP, a: (), b: ()) {
    }
}
