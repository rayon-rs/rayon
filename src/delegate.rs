//! Macros for delegating newtype iterators to inner types.

// Note: these place `impl` bounds at the end, as token gobbling is the only way
// I know how to consume an arbitrary list of constraints, with `$($args:tt)*`.

/// Creates a parallel iterator implementation which simply wraps an inner type
/// and delegates all methods inward.  The actual struct must already be
/// declared with an `inner` field.
///
/// The implementation of `IntoParallelIterator` should be added separately.
///
/// # Example
///
/// ```
/// delegate_iterator!{
///     MyIntoIter<T, U> => (T, U),
///     impl<T: Ord + Send, U: Send>
/// }
/// ```
macro_rules! delegate_iterator {
    ($iter:ty => $item:ty ,
     impl $( $args:tt )*
     ) => {
        impl $( $args )* ParallelIterator for $iter {
            type Item = $item;

            fn drive_unindexed<C>(self, consumer: C) -> C::Result
                where C: UnindexedConsumer<Self::Item>
            {
                self.inner.drive_unindexed(consumer)
            }

            fn opt_len(&self) -> Option<usize> {
                self.inner.opt_len()
            }
        }
    }
}

/// Creates an indexed parallel iterator implementation which simply wraps an
/// inner type and delegates all methods inward.  The actual struct must already
/// be declared with an `inner` field.
macro_rules! delegate_indexed_iterator {
    ($iter:ty => $item:ty ,
     impl $( $args:tt )*
     ) => {
        delegate_iterator!{
            $iter => $item ,
            impl $( $args )*
        }

        impl $( $args )* IndexedParallelIterator for $iter {
            fn drive<C>(self, consumer: C) -> C::Result
                where C: Consumer<Self::Item>
            {
                self.inner.drive(consumer)
            }

            fn len(&self) -> usize {
                self.inner.len()
            }

            fn with_producer<CB>(self, callback: CB) -> CB::Output
                where CB: ProducerCallback<Self::Item>
            {
                self.inner.with_producer(callback)
            }
        }
    }
}
