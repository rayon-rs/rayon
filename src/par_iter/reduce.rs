use std;
use super::ParallelIterator;
use super::len::*;
use super::internal::*;

/// Specifies a "reduce operator". This is the combination of a start
/// value and a reduce function. The reduce function takes two items
/// and computes a reduced version. The start value `S` is a kind of
/// "zero" or "identity" value that may be intermingled as needed;
/// ideally, `reduce(S, X)` for any item `X` yields `X`.
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
    let consumer = ReduceConsumer { reduce_op: reduce_op };
    pi.drive_unindexed(consumer)
}

struct ReduceConsumer<'r, REDUCE_OP: 'r> {
    reduce_op: &'r REDUCE_OP,
}

impl<'r, REDUCE_OP> Copy for ReduceConsumer<'r, REDUCE_OP> {
}

impl<'r, REDUCE_OP> Clone for ReduceConsumer<'r, REDUCE_OP> {
    fn clone(&self) -> Self { *self }
}

impl<'r, REDUCE_OP, ITEM> Consumer<ITEM> for ReduceConsumer<'r, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>,
          ITEM: Send,
{
    type Folder = ReduceFolder<'r, REDUCE_OP, ITEM>;
    type Reducer = Self;
    type Result = ITEM;

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, Self) {
        (self, self, self)
    }

    fn into_folder(self) -> ReduceFolder<'r, REDUCE_OP, ITEM> {
        ReduceFolder { reduce_op: self.reduce_op,
                       item: self.reduce_op.start_value() }
    }

    fn should_continue(&self) -> bool {
        true
    }
}

impl<'r, REDUCE_OP, ITEM> UnindexedConsumer<ITEM> for ReduceConsumer<'r, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>,
          ITEM: Send,
{
    fn split_off(&self) -> Self {
        ReduceConsumer { reduce_op: self.reduce_op }
    }

    fn to_reducer(&self) -> Self::Reducer {
        *self
    }
}

impl<'r, REDUCE_OP, ITEM> Reducer<ITEM> for ReduceConsumer<'r, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>,
{
    fn reduce(self, left: ITEM, right: ITEM) -> ITEM {
        self.reduce_op.reduce(left, right)
    }
}

pub struct ReduceFolder<'r, REDUCE_OP: 'r, ITEM> {
    reduce_op: &'r REDUCE_OP,
    item: ITEM,
}

impl<'r, REDUCE_OP, ITEM> Folder<ITEM> for ReduceFolder<'r, REDUCE_OP, ITEM>
    where REDUCE_OP: ReduceOp<ITEM>,
{
    type Result = ITEM;

    fn consume(self, item: ITEM) -> Self {
        let item = self.reduce_op.reduce(self.item, item);
        ReduceFolder { reduce_op: self.reduce_op, item: item }
    }

    fn complete(self) -> ITEM {
        self.item
    }

    fn should_continue(&self) -> bool {
        true
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

pub struct ReduceWithOp<'r, OP: 'r> {
    op: &'r OP
}

impl<'r, OP> ReduceWithOp<'r, OP> {
    pub fn new(op: &'r OP) -> ReduceWithOp<'r, OP> {
        ReduceWithOp {
            op: op
        }
    }
}

impl<'r, ITEM,OP> ReduceOp<Option<ITEM>> for ReduceWithOp<'r, OP>
    where OP: Fn(ITEM, ITEM) -> ITEM + Sync + 'r
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

pub struct ReduceWithIdentityOp<'r, ITEM: 'r, OP: 'r> {
    identity: &'r ITEM,
    op: &'r OP,
}

impl<'r, ITEM, OP> ReduceWithIdentityOp<'r, ITEM, OP> {
    pub fn new(identity: &'r ITEM, op: &'r OP) -> ReduceWithIdentityOp<'r, ITEM, OP> {
        ReduceWithIdentityOp {
            identity: identity,
            op: op,
        }
    }
}

impl<'r, ITEM, OP> ReduceOp<ITEM> for ReduceWithIdentityOp<'r, ITEM, OP>
    where OP: Fn(ITEM, ITEM) -> ITEM + Sync,
          ITEM: 'r + Clone + Sync,
{
    fn start_value(&self) -> ITEM {
        self.identity.clone()
    }

    fn reduce(&self, value1: ITEM, value2: ITEM) -> ITEM {
        (self.op)(value1, value2)
    }
}
