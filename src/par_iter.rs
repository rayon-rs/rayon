use std::iter;
use std::ops::{Fn, FnMut};
use std::ptr;

pub trait ParallelIterator {
    type Dim: Dimension;
    type State: ParallelIteratorState;

    fn into_state(self) -> Self::State;
}

pub trait ParallelIteratorState {
    type Item;
    type Split: ParallelIteratorState<Item=Self::Item>;
    type Iter: Iterator<Item=Self::Item>;

    unsafe fn len(&mut self) -> ParallelLen;

    fn split_at(self, index: usize) -> (Self::Split, Self::Split);

    fn into_iter(self) -> Self::Iter;

    /// Initializes some number of consecutive entries at
    /// `target`. Returns number that were initialized.
    unsafe fn initialize(self, mut target: *mut Self::Item) -> usize {
        let mut counter = 0;
        for item in self.into_iter() {
            std::ptr::write(target, item);
            target = target.offset(1);
            counter += 1;
        }
        counter
    }
}

pub trait ParHandle<E>: Clone + Send + Sync {
    fn initialize(handle: Self::Handle, index: usize) -> *mut E;
    fn split_at(self, index: usize) -> (Self::Split, Self::Split);
}

pub struct ParallelLen {
    /// Maximal number of elements that we will write
    pub maximal_len: usize,

    /// An estimate of the "cost" of this operation. This is a kind of
    /// abstract concept you can use to influence how fine-grained the
    /// threads are.
    ///
    /// TODO: refine this metric.
    pub cost: f64,

    /// If true, all elements will be written. If false, some may not.
    /// For example, `sparse` will be false if there is a filter.
    /// When doing a collect, sparse iterators require a compression
    /// step.
    pub sparse: bool,
}

///////////////////////////////////////////////////////////////////////////

trait Dimension {
    type Index;
    type Item;
}

struct OneDimensional<T> {
    phantom: PhantomData<*mut T>
}

impl<T> OneDimensional<T> {
    pub fn new() -> OneDimensional<T> {
        OneDimensional { phanton: PhantomData }
    }
}

impl<T> Dimension for OneDimensional<T> {
    type Index = usize;
    type Item = T;
}

struct MoreDimensional<D: Dimension> {
    phantom: PhantomData<*mut D>
}

impl<T> MoreDimensional<T> {
    pub fn new() -> MoreDimensional<T> {
        MoreDimensional { phanton: PhantomData }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct SliceIterator<'data, T> {
    slice: &'data [T]
}

impl<'data, T> ParallelIterator for SliceIterator<'data, T> {
    type Dim = OneDimensional<T>;
}

///////////////////////////////////////////////////////////////////////////

pub struct MapIterator<'map, M, OP>
    where M: ParallelIterator, OP: Fn<(M::Item,)>
{
    base: M,
    op: &'map OP,
}

impl<'map, M, OP> ParallelIterator for MapIterator<'data, M, OP> {
    type Dim = OneDimensional<T>;
    type State = ParallelIteratorState<'data, M, OP>;

    pub fn into_state(self) -> Self::State {
        MapIteratorState {
            base: self.base.into_state(),
            op: self.op,
        }
    }
}

pub struct MapIteratorState<'map, M, OP>
    where M: ParallelIteratorState, OP: Fn<(M::Item,)>
{
    base: M,
    op: &'map OP,
}

impl<'map, M, OP> ParallelIteratorState for MapIterator<'map, M, OP>
    where M: ParallelIteratorState, OP: Fn<(M::Item,)>
{
    type Item = OP::Output;
    type Split = MapIterator<'map, M::Split, OP>;
    type Iter = iter::Map<M::Iter, &'map OP>;

    pub unsafe fn len(&mut self) -> ParallelLen {
        self.base.len()
    }

    pub fn into_iter(self) -> Self::Iter {
        self.base.into_iter().map(self.op)
    }
}

///////////////////////////////////////////////////////////////////////////

pub trait ParCollect<E> {
    type Handle: ParHandle<E>;

    fn create(len: ParallelLen) -> Self::Handle;
    fn complete(handle: Self::Handle) -> Self;
}

fn collect_into<PI,E,D,C>(pi: PI) -> C
    where PI: ParallelIterator<Dim=D>,
          C: ParCollect<E>,
{
    let mut state = pi.into_state();
    let len = state.len();
    let mut handle = C::create(len);
    state.execute_par(handle);
    C::complete(handle)
}
