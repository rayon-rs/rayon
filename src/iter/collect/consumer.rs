use super::super::plumbing::*;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr;
use std::slice;

pub(super) struct CollectConsumer<'c, T: Send> {
    /// A slice covering the target memory, not yet initialized!
    target: &'c mut [MaybeUninit<T>],
}

impl<'c, T: Send + 'c> CollectConsumer<'c, T> {
    /// The target memory is considered uninitialized, and will be
    /// overwritten without reading or dropping existing values.
    pub(super) fn new(target: &'c mut [MaybeUninit<T>]) -> Self {
        CollectConsumer { target }
    }
}

/// CollectResult represents an initialized part of the target slice.
///
/// This is a proxy owner of the elements in the slice; when it drops,
/// the elements will be dropped, unless its ownership is released before then.
#[must_use]
pub(super) struct CollectResult<'c, T> {
    /// A slice covering the target memory, initialized up to our separate `len`.
    target: &'c mut [MaybeUninit<T>],
    /// The current initialized length in `target`
    len: usize,
    /// Lifetime invariance guarantees that the data flows from consumer to result,
    /// especially for the `scope_fn` callback in `Collect::with_consumer`.
    invariant_lifetime: PhantomData<&'c mut &'c mut [T]>,
}

unsafe impl<'c, T> Send for CollectResult<'c, T> where T: Send {}

impl<'c, T> CollectResult<'c, T> {
    /// The current length of the collect result
    pub(super) fn len(&self) -> usize {
        self.len
    }

    /// Release ownership of the slice of elements, and return the length
    pub(super) fn release_ownership(mut self) -> usize {
        let ret = self.len;
        self.len = 0;
        ret
    }
}

impl<'c, T> Drop for CollectResult<'c, T> {
    fn drop(&mut self) {
        // Drop the first `self.len` elements, which have been recorded
        // to be initialized by the folder.
        unsafe {
            // TODO: use `MaybeUninit::slice_as_mut_ptr`
            let start = self.target.as_mut_ptr() as *mut T;
            ptr::drop_in_place(slice::from_raw_parts_mut(start, self.len));
        }
    }
}

impl<'c, T: Send + 'c> Consumer<T> for CollectConsumer<'c, T> {
    type Folder = CollectResult<'c, T>;
    type Reducer = CollectReducer;
    type Result = CollectResult<'c, T>;

    fn split_at(self, index: usize) -> (Self, Self, CollectReducer) {
        let CollectConsumer { target } = self;

        // Produce new consumers. Normal slicing ensures that the
        // memory range given to each consumer is disjoint.
        let (left, right) = target.split_at_mut(index);
        (
            CollectConsumer::new(left),
            CollectConsumer::new(right),
            CollectReducer,
        )
    }

    fn into_folder(self) -> Self::Folder {
        // Create a result/folder that consumes values and writes them
        // into target. The initial result has length 0.
        CollectResult {
            target: self.target,
            len: 0,
            invariant_lifetime: PhantomData,
        }
    }

    fn full(&self) -> bool {
        false
    }
}

impl<'c, T: Send + 'c> Folder<T> for CollectResult<'c, T> {
    type Result = Self;

    fn consume(mut self, item: T) -> Self {
        let dest = self
            .target
            .get_mut(self.len)
            .expect("too many values pushed to consumer");

        // Write item and increase the initialized length
        unsafe {
            dest.as_mut_ptr().write(item);
            self.len += 1;
        }

        self
    }

    fn complete(self) -> Self::Result {
        // NB: We don't explicitly check that the local writes were complete,
        // but Collect will assert the total result length in the end.
        self
    }

    fn full(&self) -> bool {
        false
    }
}

/// Pretend to be unindexed for `special_collect_into_vec`,
/// but we should never actually get used that way...
impl<'c, T: Send + 'c> UnindexedConsumer<T> for CollectConsumer<'c, T> {
    fn split_off_left(&self) -> Self {
        unreachable!("CollectConsumer must be indexed!")
    }
    fn to_reducer(&self) -> Self::Reducer {
        CollectReducer
    }
}

/// CollectReducer combines adjacent chunks; the result must always
/// be contiguous so that it is one combined slice.
pub(super) struct CollectReducer;

impl<'c, T> Reducer<CollectResult<'c, T>> for CollectReducer {
    fn reduce(
        self,
        mut left: CollectResult<'c, T>,
        right: CollectResult<'c, T>,
    ) -> CollectResult<'c, T> {
        // Merge if the CollectResults are adjacent and in left to right order
        // else: drop the right piece now and total length will end up short in the end,
        // when the correctness of the collected result is asserted.
        let left_end = left.target[left.len..].as_ptr();
        if left_end == right.target.as_ptr() {
            let len = left.len + right.release_ownership();
            unsafe {
                left.target = slice::from_raw_parts_mut(left.target.as_mut_ptr(), len);
            }
            left.len = len;
        }
        left
    }
}
