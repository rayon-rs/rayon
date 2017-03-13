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

pub fn reduce<PAR_ITER, R, T>(pi: PAR_ITER, reduce_op: &R) -> T
    where PAR_ITER: ParallelIterator<Item = T>,
          R: ReduceOp<T>,
          T: Send
{
    let consumer = ReduceConsumer { reduce_op: reduce_op };
    pi.drive_unindexed(consumer)
}

struct ReduceConsumer<'r, R: 'r> {
    reduce_op: &'r R,
}

impl<'r, R> Copy for ReduceConsumer<'r, R> {}

impl<'r, R> Clone for ReduceConsumer<'r, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'r, R, I> Consumer<I> for ReduceConsumer<'r, R>
    where R: ReduceOp<I>,
          I: Send
{
    type Folder = ReduceFolder<'r, R, I>;
    type Reducer = Self;
    type Result = I;

    fn cost(&mut self, cost: f64) -> f64 {
        // This isn't quite right, as we will do more than O(n) reductions, but whatever.
        cost * FUNC_ADJUSTMENT
    }

    fn split_at(self, _index: usize) -> (Self, Self, Self) {
        (self, self, self)
    }

    fn into_folder(self) -> ReduceFolder<'r, R, I> {
        ReduceFolder {
            reduce_op: self.reduce_op,
            item: self.reduce_op.start_value(),
        }
    }
}

impl<'r, R, I> UnindexedConsumer<I> for ReduceConsumer<'r, R>
    where R: ReduceOp<I>,
          I: Send
{
    fn split_off_left(&self) -> Self {
        ReduceConsumer { reduce_op: self.reduce_op }
    }

    fn to_reducer(&self) -> Self::Reducer {
        *self
    }
}

impl<'r, R, I> Reducer<I> for ReduceConsumer<'r, R>
    where R: ReduceOp<I>
{
    fn reduce(self, left: I, right: I) -> I {
        self.reduce_op.reduce(left, right)
    }
}

struct ReduceFolder<'r, R: 'r, I> {
    reduce_op: &'r R,
    item: I,
}

impl<'r, R, I> Folder<I> for ReduceFolder<'r, R, I>
    where R: ReduceOp<I>
{
    type Result = I;

    fn consume(self, item: I) -> Self {
        let item = self.reduce_op.reduce(self.item, item);
        ReduceFolder {
            reduce_op: self.reduce_op,
            item: item,
        }
    }

    fn complete(self) -> I {
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

pub struct ReduceWithIdentityOp<'r, ID: 'r, OP: 'r> {
    identity: &'r ID,
    op: &'r OP,
}

impl<'r, ID, OP> ReduceWithIdentityOp<'r, ID, OP> {
    pub fn new(identity: &'r ID, op: &'r OP) -> ReduceWithIdentityOp<'r, ID, OP> {
        ReduceWithIdentityOp {
            identity: identity,
            op: op,
        }
    }
}

impl<'r, ID, OP, I> ReduceOp<I> for ReduceWithIdentityOp<'r, ID, OP>
    where OP: Fn(I, I) -> I + Sync,
          ID: Fn() -> I + Sync,
          I: 'r
{
    fn start_value(&self) -> I {
        (self.identity)()
    }

    fn reduce(&self, value1: I, value2: I) -> I {
        (self.op)(value1, value2)
    }
}
