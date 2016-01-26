use std;
use super::ParallelIterator;
use super::len::*;
use super::state::*;
use super::util::PhantomType;

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

pub fn reduce<PAR_ITER,REDUCE_OP,T>(pi: PAR_ITER, reduce_op: &REDUCE_OP) -> T
    where PAR_ITER: ParallelIterator<Item=T>,
          REDUCE_OP: ReduceOp<T>,
          T: Send,
{
    let consumer: ReduceConsumer<PAR_ITER::Item, REDUCE_OP> =
        ReduceConsumer { data: PhantomType::new() };
    pi.drive_unindexed(consumer, reduce_op)
}

struct ReduceConsumer<ITEM, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>,
{
    data: PhantomType<(ITEM, REDUCE_OP)>
}

impl<ITEM, REDUCE_OP> Copy for ReduceConsumer<ITEM, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>,
{
}

impl<ITEM, REDUCE_OP> Clone for ReduceConsumer<ITEM, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>,
{
    fn clone(&self) -> Self { *self }
}

impl<'c, ITEM, REDUCE_OP> Consumer<'c> for ReduceConsumer<ITEM, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM> + 'c,
          ITEM: Send + 'c,
{
    type Item = ITEM;
    type Shared = REDUCE_OP;
    type SeqState = ITEM;
    type Result = ITEM;

    fn cost(&mut self, _shared: &Self::Shared, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, _: &Self::Shared, _index: usize) -> (Self, Self) {
        (self, self)
    }

    unsafe fn start(&mut self, reduce_op: &REDUCE_OP) -> ITEM {
        reduce_op.start_value()
    }

    unsafe fn consume(&mut self,
                      reduce_op: &REDUCE_OP,
                      prev_value: ITEM,
                      item: ITEM)
                      -> ITEM {
        reduce_op.reduce(prev_value, item)
    }

    unsafe fn complete(self,
                       _reduce_op: &REDUCE_OP,
                       state: ITEM)
                       -> ITEM {
        state
    }

    unsafe fn reduce(reduce_op: &REDUCE_OP,
                     a: ITEM,
                     b: ITEM)
                     -> ITEM {
        reduce_op.reduce(a, b)
    }
}

impl<'c, ITEM, REDUCE_OP> UnindexedConsumer<'c> for ReduceConsumer<ITEM, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM> + 'c,
          ITEM: Send + 'c,
{
    fn split(&self) -> Self {
        ReduceConsumer { data: PhantomType::new() }
    }
}

///////////////////////////////////////////////////////////////////////////
// Specific operations

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

pub struct MinOp;

pub const MIN: &'static MinOp = &MinOp;

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

pub const MAX: &'static MaxOp = &MaxOp;

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
