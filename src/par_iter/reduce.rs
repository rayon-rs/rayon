use api::join;
use std;
use std::marker::PhantomData;
use super::ParallelIterator;
use super::len::*;
use super::state::*;

/// Specifies a "reduce operator". This is the combination of a start
/// value and a reduce function. The reduce function takes two items
/// and computes a reduced version. The start value `S` is a kind of
/// "zero" or "identity" value that may be intermingled as needed;
/// idealy, `reduce(S, X)` for any item `X` yields `X`.
///
/// Example: to sum up the values, use a `start_value` of `0` and a
/// reduce function of `reduce(a, b) = a + b`.
///
/// The order in which the reduce function will be applied is not
/// specified. For example, the input `[ 0 1 2 ]` might be reduced in a
/// sequential fashion:
///
/// ```ignore
/// reduce(reduce(reduce(S, 0), 1), 2)
/// ```
///
/// or it might be reduced in a tree-like way:
///
/// ```ignore
/// reduce(reduce(0, 1), reduce(S, 2))
/// ```
///
/// etc.
pub trait ReduceOp<T>: Sync {
    fn start_value(&self) -> T;
    fn reduce(&self, value1: T, value2: T) -> T;
}

pub fn reduce<PAR_ITER,REDUCE_OP,T>(pi: PAR_ITER, reduce_op: REDUCE_OP) -> T
    where PAR_ITER: ParallelIterator<Item=T>,
          PAR_ITER::State: Send,
          REDUCE_OP: ReduceOp<T>,
          T: Send,
{
    let consumer: ReduceConsumer<PAR_ITER, REDUCE_OP> = ReduceConsumer::new();
    pi.drive(consumer, reduce_op)
}

struct ReduceConsumer<PAR_ITER, REDUCE_OP>
    where PAR_ITER: ParallelIterator,
          REDUCE_OP: ReduceOp<PAR_ITER::Item>,
{
    data: PhantomData<(PAR_ITER, REDUCE_OP)>
}

impl<PAR_ITER, REDUCE_OP> ReduceConsumer<PAR_ITER, REDUCE_OP>
    where PAR_ITER: ParallelIterator,
          REDUCE_OP: ReduceOp<PAR_ITER::Item>,
{
    fn new() -> ReduceConsumer<PAR_ITER, REDUCE_OP> {
        ReduceConsumer { data: PhantomData }
    }
}

unsafe impl<PAR_ITER, REDUCE_OP> Send for ReduceConsumer<PAR_ITER, REDUCE_OP>
    where PAR_ITER: ParallelIterator,
          REDUCE_OP: ReduceOp<PAR_ITER::Item>,
{ }

impl<PAR_ITER, REDUCE_OP> Consumer for ReduceConsumer<PAR_ITER, REDUCE_OP>
    where PAR_ITER: ParallelIterator,
          REDUCE_OP: ReduceOp<PAR_ITER::Item>,
{
    type Item = PAR_ITER::Item;
    type Shared = REDUCE_OP;
    type SeqState = PAR_ITER::Item;
    type Result = PAR_ITER::Item;

    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, _: &Self::Shared, _index: usize) -> (Self, Self) {
        (ReduceConsumer::new(), ReduceConsumer::new())
    }

    unsafe fn start(&mut self, reduce_op: &REDUCE_OP) -> PAR_ITER::Item {
        reduce_op.start_value()
    }

    unsafe fn consume(&mut self,
                      reduce_op: &REDUCE_OP,
                      prev_value: PAR_ITER::Item,
                      item: PAR_ITER::Item)
                      -> PAR_ITER::Item {
        reduce_op.reduce(prev_value, item)
    }

    unsafe fn complete(self,
                       _reduce_op: &REDUCE_OP,
                       state: PAR_ITER::Item)
                       -> PAR_ITER::Item {
        state
    }

    unsafe fn reduce(reduce_op: &REDUCE_OP,
                     a: PAR_ITER::Item,
                     b: PAR_ITER::Item)
                     -> PAR_ITER::Item {
        reduce_op.reduce(a, b)
    }
}

///////////////////////////////////////////////////////////////////////////
// Specific operations

pub struct SumOp;

pub const SUM: SumOp = SumOp;

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

pub const MUL: MulOp = MulOp;

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

pub struct MinOp;

pub const MIN: MinOp = MinOp;

macro_rules! min_rule {
    ($i:ty, $z:expr, $f:expr) => {
        impl ReduceOp<$i> for MinOp {
            fn start_value(&self) -> $i {
                $z
            }
            fn reduce(&self, value1: $i, value2: $i) -> $i {
                $f(value1, value2)
            }
        }
    }
}

min_rule!(i8, std::i8::MAX, std::cmp::min);
min_rule!(i16, std::i16::MAX, std::cmp::min);
min_rule!(i32, std::i32::MAX, std::cmp::min);
min_rule!(i64, std::i64::MAX, std::cmp::min);
min_rule!(isize, std::isize::MAX, std::cmp::min);
min_rule!(u8, std::u8::MAX, std::cmp::min);
min_rule!(u16, std::u16::MAX, std::cmp::min);
min_rule!(u32, std::u32::MAX, std::cmp::min);
min_rule!(u64, std::u64::MAX, std::cmp::min);
min_rule!(usize, std::usize::MAX, std::cmp::min);
min_rule!(f32, std::f32::INFINITY, f32::min);
min_rule!(f64, std::f64::INFINITY, f64::min);

pub struct MaxOp;

pub const MAX: MaxOp = MaxOp;

macro_rules! max_rule {
    ($i:ty, $z:expr, $f:expr) => {
        impl ReduceOp<$i> for MaxOp {
            fn start_value(&self) -> $i {
                $z
            }
            fn reduce(&self, value1: $i, value2: $i) -> $i {
                $f(value1, value2)
            }
        }
    }
}

max_rule!(i8, std::i8::MIN, std::cmp::max);
max_rule!(i16, std::i16::MIN, std::cmp::max);
max_rule!(i32, std::i32::MIN, std::cmp::max);
max_rule!(i64, std::i64::MIN, std::cmp::max);
max_rule!(isize, std::isize::MIN, std::cmp::max);
max_rule!(u8, std::u8::MIN, std::cmp::max);
max_rule!(u16, std::u16::MIN, std::cmp::max);
max_rule!(u32, std::u32::MIN, std::cmp::max);
max_rule!(u64, std::u64::MIN, std::cmp::max);
max_rule!(usize, std::usize::MIN, std::cmp::max);
max_rule!(f32, std::f32::NEG_INFINITY, f32::max);
max_rule!(f64, std::f64::NEG_INFINITY, f64::max);

pub struct ReduceWithOp<OP> {
    op: OP
}

impl<OP> ReduceWithOp<OP> {
    pub fn new(op: OP) -> ReduceWithOp<OP> {
        ReduceWithOp {
            op: op
        }
    }
}

impl<ITEM,OP> ReduceOp<Option<ITEM>> for ReduceWithOp<OP>
    where OP: Fn(ITEM, ITEM) -> ITEM + Sync
{
    fn start_value(&self) -> Option<ITEM> {
        None
    }

    fn reduce(&self, value1: Option<ITEM>, value2: Option<ITEM>) -> Option<ITEM> {
        if let Some(value1) = value1 {
            if let Some(value2) = value2 {
                Some((self.op)(value1, value2))
            } else {
                Some(value1)
            }
        } else {
            value2
        }
    }
}
