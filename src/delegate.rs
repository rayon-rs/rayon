//! Macros for delegating newtype iterators to inner types.

// Note: these place `impl` bounds at the end, as token gobbling is the only way
// I know how to consume an arbitrary list of constraints, with `$($args:tt)*`.

/// Create a parallel iterator which simply wraps an inner type and delegates
/// all methods inward.  The item type is parsed from the inner type.
///
/// The implementation of `IntoParallelIterator` should be added separately.
///
/// # Example
///
/// ```
/// delegate_iterator!{
///     #[doc = "Move items from `MyCollection` in parallel"]
///     MyIntoIter<T, U> => vec::IntoIter<(T, U)>,
///     impl<T: Ord + Send, U: Send>
/// }
/// ```
macro_rules! delegate_iterator {
    ($( #[ $attr:meta ] )+
     $iter:ident < $( $i:tt ),* > => $( $inner:ident )::+ < $item:ty > ,
     impl $( $args:tt )*
     ) => {
        delegate_iterator_item!{
            $( #[ $attr ] )+
            $iter < $( $i ),* > => $( $inner )::+ < $item > : $item ,
            impl $( $args )*
        }
    }
}

/// Create an indexed parallel iterator which simply wraps an inner type and
/// delegates all methods inward.  The item type is parsed from the inner type.
macro_rules! delegate_indexed_iterator {
    ($( #[ $attr:meta ] )+
     $iter:ident < $( $i:tt ),* > => $( $inner:ident )::+ < $item:ty > ,
     impl $( $args:tt )*
     ) => {
        delegate_indexed_iterator_item!{
            $( #[ $attr ] )+
            $iter < $( $i ),* > => $( $inner )::+ < $item > : $item ,
            impl $( $args )*
        }
    }
}

/// Create a parallel iterator which simply wraps an inner type and delegates
/// all methods inward.  The item type is explicitly specified.
///
/// The implementation of `IntoParallelIterator` should be added separately.
///
/// # Example
///
/// ```
/// delegate_iterator_item!{
///     #[doc = "Iterate items from `MyCollection` in parallel"]
///     MyIter<'a, T, U> => slice::Iter<'a, (T, U)>: &'a (T, U),
///     impl<'a, T: Ord + Sync, U: Sync>
/// }
/// ```
macro_rules! delegate_iterator_item {
    ($( #[ $attr:meta ] )+
     $iter:ident < $( $i:tt ),* > => $inner:ty : $item:ty,
     impl $( $args:tt )*
     ) => {
        $( #[ $attr ] )+
        pub struct $iter $( $args )* {
            inner: $inner,
        }

        impl $( $args )* ParallelIterator for $iter < $( $i ),* > {
            type Item = $item;

            fn drive_unindexed<C>(self, consumer: C) -> C::Result
                where C: UnindexedConsumer<Self::Item>
            {
                self.inner.drive_unindexed(consumer)
            }

            fn opt_len(&mut self) -> Option<usize> {
                self.inner.opt_len()
            }
        }
    }
}

/// Create an indexed parallel iterator which simply wraps an inner type and
/// delegates all methods inward.  The item type is explicitly specified.
macro_rules! delegate_indexed_iterator_item {
    ($( #[ $attr:meta ] )+
     $iter:ident < $( $i:tt ),* > => $inner:ty : $item:ty,
     impl $( $args:tt )*
     ) => {
        delegate_iterator_item!{
            $( #[ $attr ] )+
            $iter < $( $i ),* > => $inner : $item ,
            impl $( $args )*
        }

        impl $( $args )* IndexedParallelIterator for $iter < $( $i ),* > {
            fn drive<C>(self, consumer: C) -> C::Result
                where C: Consumer<Self::Item>
            {
                self.inner.drive(consumer)
            }

            fn len(&mut self) -> usize {
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
