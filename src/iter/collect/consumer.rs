use super::super::plumbing::*;
use std::marker::PhantomData;
use std::ptr;
use std::slice;

pub(super) struct CollectConsumer<'c, T: Send> {
    /// A slice covering the target memory, not yet initialized!
    target: &'c mut [T],
}

pub(super) struct CollectFolder<'c, T: Send> {
    /// The folder writes into `result` and must extend the result
    /// up to exactly this number of elements.
    final_len: usize,

    /// The current written-to part of our slice of the target
    result: CollectResult<'c, T>,
}

impl<'c, T: Send + 'c> CollectConsumer<'c, T> {
    /// The target memory is considered uninitialized, and will be
    /// overwritten without reading or dropping existing values.
    pub(super) fn new(target: &'c mut [T]) -> Self {
        CollectConsumer { target }
    }
}

/// CollectResult represents an initialized part of the target slice.
///
/// This is a proxy owner of the elements in the slice; when it drops,
/// the elements will be dropped, unless its ownership is released before then.
#[must_use]
pub(super) struct CollectResult<'c, T> {
    start: *mut T,
    len: usize,
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
            ptr::drop_in_place(slice::from_raw_parts_mut(self.start, self.len));
        }
    }
}

impl<'c, T: Send + 'c> Consumer<T> for CollectConsumer<'c, T> {
    type Folder = CollectFolder<'c, T>;
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

    fn into_folder(self) -> CollectFolder<'c, T> {
        // Create a folder that consumes values and writes them
        // into target. The initial result has length 0.
        CollectFolder {
            final_len: self.target.len(),
            result: CollectResult {
                start: self.target.as_mut_ptr(),
                len: 0,
                invariant_lifetime: PhantomData,
            },
        }
    }

    fn full(&self) -> bool {
        false
    }
}

impl<'c, T: Send + 'c> Folder<T> for CollectFolder<'c, T> {
    type Result = CollectResult<'c, T>;

    fn consume(mut self, item: T) -> CollectFolder<'c, T> {
        if self.result.len >= self.final_len {
            panic!("too many values pushed to consumer");
        }

        // Compute target pointer and write to it, and
        // extend the current result by one element
        unsafe {
            self.result.start.add(self.result.len).write(item);
            self.result.len += 1;
        }

        self
    }

    fn complete(self) -> Self::Result {
        // NB: We don't explicitly check that the local writes were complete,
        // but Collect will assert the total result length in the end.
        self.result
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
        if left.start.wrapping_add(left.len) == right.start {
            left.len += right.release_ownership();
        }
        left
    }
}
