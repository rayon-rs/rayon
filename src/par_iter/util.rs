use super::internal::Reducer;
use std::marker::PhantomData;

/// A little structure used to mark some type. This is often used when
/// we have some phantom type we wish to remember as part of a struct.
/// This doesn't represent "reachable data", and hence it is
/// considered sendable regardless of `T`, unlike `PhantomData`.
pub struct PhantomType<T> {
    data: PhantomData<T>
}

unsafe impl<T> Send for PhantomType<T> { }

unsafe impl<T> Sync for PhantomType<T> { }

impl<T> Copy for PhantomType<T> { }

impl<T> Clone for PhantomType<T> {
    fn clone(&self) -> Self { *self }
}

impl<T> PhantomType<T> {
    pub fn new() -> PhantomType<T> {
        PhantomType { data: PhantomData }
    }
}

/// Utility type for consumers that don't need a "reduce" step.
pub struct NoopReducer;

impl Reducer for NoopReducer {
    type Result = ();
    fn reduce(self, _left: (), _right: ()) { }
}

