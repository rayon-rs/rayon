use api::join;
use super::ParallelIterator;
use super::len::{ParallelLen, THRESHOLD};
use super::state::ParallelIteratorState;

pub fn for_each<PAR_ITER,OP,T>(pi: PAR_ITER, op: &OP)
    where PAR_ITER: ParallelIterator<Item=T>,
          PAR_ITER::State: Send,
          OP: Fn(T) + Sync,
          T: Send,
{
    let (shared, mut state) = pi.state();
    let len = state.len(&shared);
    for_each_helper(state, &shared, len, op)
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

