use api::join;
use super::{ParallelIterator, ParallelIteratorState, ParallelLen, THRESHOLD};

pub trait ReduceOp<T>: Sync {
    fn start_value(&self) -> T;
    fn reduce(&self, value1: T, value2: T) -> T;
}

pub fn reduce<PAR_ITER,REDUCE_OP,T>(pi: PAR_ITER, reduce_op: &REDUCE_OP) -> T
    where PAR_ITER: ParallelIterator<Item=T>,
          PAR_ITER::State: Send,
          REDUCE_OP: ReduceOp<T>,
{
    let (shared, mut state) = pi.state();
    let len = state.len();
    reduce_helper(state, &shared, len, reduce_op)
}

fn reduce_helper<STATE,REDUCE_OP,T>(state: STATE,
                                    shared: &STATE::Shared,
                                    len: ParallelLen,
                                    reduce_op: &REDUCE_OP)
                                    -> T
    where STATE: ParallelIteratorState<Item=T> + Send,
          REDUCE_OP: ReduceOp<T>
{
    if len.cost > THRESHOLD && len.maximal_len > 1 {
        let mid = len.maximal_len / 2;
        let (left, right) = state.split_at(mid);
        let (left_val, right_val) =
            join(|| reduce_helper(left, shared, len.left_cost(mid), reduce_op),
                 || reduce_helper(right, shared, len.right_cost(mid), reduce_op));
        reduce_op.reduce(left_val, right_val)
    } else {
        let mut value = Some(reduce_op.start_value());
        state.for_each(shared, |item| {
            value = Some(reduce_op.reduce(value.take().unwrap(), item));
        });
        value.unwrap()
    }
}

pub struct SumOp;

pub const SUM: &'static SumOp = &SumOp;

macro_rules! sum_rule {
    ($i:ty, $z:expr) => {
        impl ReduceOp<$i> for SumOp {
            fn start_value(&self) -> $i {
                $z
            }
            fn reduce(&self, value1: $i, value2: $i) -> $i {
                value1 + value2
            }
        }
    }
}

sum_rule!(i8, 0);
sum_rule!(i16, 0);
sum_rule!(i32, 0);
sum_rule!(i64, 0);
sum_rule!(isize, 0);
sum_rule!(u8, 0);
sum_rule!(u16, 0);
sum_rule!(u32, 0);
sum_rule!(u64, 0);
sum_rule!(usize, 0);
sum_rule!(f32, 0.0);
sum_rule!(f64, 0.0);

pub struct MulOp;

pub const MUL: &'static MulOp = &MulOp;

macro_rules! mul_rule {
    ($i:ty, $z:expr) => {
        impl ReduceOp<$i> for MulOp {
            fn start_value(&self) -> $i {
                $z
            }
            fn reduce(&self, value1: $i, value2: $i) -> $i {
                value1 * value2
            }
        }
    }
}

mul_rule!(i8, 1);
mul_rule!(i16, 1);
mul_rule!(i32, 1);
mul_rule!(i64, 1);
mul_rule!(isize, 1);
mul_rule!(u8, 1);
mul_rule!(u16, 1);
mul_rule!(u32, 1);
mul_rule!(u64, 1);
mul_rule!(usize, 1);
mul_rule!(f32, 1.0);
mul_rule!(f64, 1.0);
