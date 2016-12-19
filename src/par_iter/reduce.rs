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

pub fn reduce<PAR_ITER, REDUCE_OP, T>(pi: PAR_ITER, reduce_op: &REDUCE_OP) -> T
    where PAR_ITER: ParallelIterator<Item = T>,
          REDUCE_OP: ReduceOp<T>,
          T: Send
{
    let consumer = ReduceConsumer { reduce_op: reduce_op };
    pi.drive_unindexed(consumer)
}

struct ReduceConsumer<'r, REDUCE_OP: 'r> {
    reduce_op: &'r REDUCE_OP,
}

impl<'r, REDUCE_OP> Copy for ReduceConsumer<'r, REDUCE_OP> {}

impl<'r, REDUCE_OP> Clone for ReduceConsumer<'r, REDUCE_OP> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'r, REDUCE_OP, ITEM> Consumer<ITEM> for ReduceConsumer<'r, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>,
          ITEM: Send
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
        ReduceFolder {
            reduce_op: self.reduce_op,
            item: self.reduce_op.start_value(),
        }
    }
}

impl<'r, REDUCE_OP, ITEM> UnindexedConsumer<ITEM> for ReduceConsumer<'r, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>,
          ITEM: Send
{
    fn split_off(&self) -> Self {
        ReduceConsumer { reduce_op: self.reduce_op }
    }

    fn to_reducer(&self) -> Self::Reducer {
        *self
    }
}

impl<'r, REDUCE_OP, ITEM> Reducer<ITEM> for ReduceConsumer<'r, REDUCE_OP>
    where REDUCE_OP: ReduceOp<ITEM>
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
    where REDUCE_OP: ReduceOp<ITEM>
{
    type Result = ITEM;

    fn consume(self, item: ITEM) -> Self {
        let item = self.reduce_op.reduce(self.item, item);
        ReduceFolder {
            reduce_op: self.reduce_op,
            item: item,
        }
    }

    fn complete(self) -> ITEM {
        self.item
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Specific operations

pub struct SumOp;

pub const SUM: &'static SumOp = &SumOp;

macro_rules! sum_rule {
    ($i:ty, $z:expr) => {
        impl ReduceOp<$i> for SumOp {
            #[inline]
            fn start_value(&self) -> $i {
                $z
            }
            #[inline]
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

pub struct ProductOp;

pub const PRODUCT: &'static ProductOp = &ProductOp;

macro_rules! product_rule {
    ($i:ty, $z:expr) => {
        impl ReduceOp<$i> for ProductOp {
            #[inline]
            fn start_value(&self) -> $i {
                $z
            }
            #[inline]
            fn reduce(&self, value1: $i, value2: $i) -> $i {
                value1 * value2
            }
        }
    }
}

product_rule!(i8, 1);
product_rule!(i16, 1);
product_rule!(i32, 1);
product_rule!(i64, 1);
product_rule!(isize, 1);
product_rule!(u8, 1);
product_rule!(u16, 1);
product_rule!(u32, 1);
product_rule!(u64, 1);
product_rule!(usize, 1);
product_rule!(f32, 1.0);
product_rule!(f64, 1.0);

pub struct ReduceWithIdentityOp<'r, IDENTITY: 'r, OP: 'r> {
    identity: &'r IDENTITY,
    op: &'r OP,
}

impl<'r, IDENTITY, OP> ReduceWithIdentityOp<'r, IDENTITY, OP> {
    pub fn new(identity: &'r IDENTITY, op: &'r OP) -> ReduceWithIdentityOp<'r, IDENTITY, OP> {
        ReduceWithIdentityOp {
            identity: identity,
            op: op,
        }
    }
}

impl<'r, IDENTITY, OP, ITEM> ReduceOp<ITEM> for ReduceWithIdentityOp<'r, IDENTITY, OP>
    where OP: Fn(ITEM, ITEM) -> ITEM + Sync,
          IDENTITY: Fn() -> ITEM + Sync,
          ITEM: 'r
{
    fn start_value(&self) -> ITEM {
        (self.identity)()
    }

    fn reduce(&self, value1: ITEM, value2: ITEM) -> ITEM {
        (self.op)(value1, value2)
    }
}
