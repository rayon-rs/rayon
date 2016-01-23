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

    /// Returns an estimate of how much work is to be done.
    ///
    /// # Safety note
    ///
    /// If sparse is false, then `maximal_len` must be precisely
    /// correct.
    fn len(&mut self, shared: &Self::Shared) -> ParallelLen;

    /// Split this state into two other states, ideally of roughly
    /// equal size.
    fn split_at(self, index: usize) -> (Self, Self);

    /// Extract the next item from this iterator state. Once this
    /// method is called, sequential iteration has begun, and the
    /// other methods will no longer be called.
    fn next(&mut self, shared: &Self::Shared) -> Option<Self::Item>;
}
