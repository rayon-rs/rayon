use super::len::ParallelLen;

/// The trait for types representing the internal *state* during
/// parallelization. This basically represents a group of tasks
/// to be done.
///
/// Note that this trait is declared as an **unsafe trait**. That
/// means that the trait is unsafe to implement. The reason is that
/// other bits of code, such as the `collect` routine on
/// `ParallelIterator`, rely on the `len` and `for_each` functions
/// being accurate and correct. For example, if the `len` function
/// reports that it will produce N items, then `for_each` *must*
/// produce `N` items or else the resulting vector will contain
/// uninitialized memory.
///
/// This trait is not really intended to be implemented outside of the
/// Rayon crate at this time. The precise safety requirements are kind
/// of ill-documented for this reason (i.e., they are ill-understood).
pub unsafe trait ParallelIteratorState: Sized {
    type Item;
    type Shared: Sync;

    fn len(&mut self, shared: &Self::Shared) -> ParallelLen;

    fn split_at(self, index: usize) -> (Self, Self);

    fn for_each<OP>(self, shared: &Self::Shared, op: OP)
        where OP: FnMut(Self::Item);
}
