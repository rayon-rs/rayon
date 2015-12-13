#![allow(dead_code)]

use api::join;
use std::isize;
use std::marker::PhantomData;
use std::ops::Fn;
use std::ptr;

#[cfg(test)]
mod test;

pub trait IntoParallelIterator {
    type Iter: ParallelIterator<Item=Self::Item>;
    type Item;

    fn into_par_iter(self) -> Self::Iter;
}

pub trait ParallelIterator {
    type Item;
    type Shared: Sync;
    type State: ParallelIteratorState<Shared=Self::Shared, Item=Self::Item>;

    fn state(self) -> (Self::Shared, Self::State);

    fn map<OP,R>(self, op: OP) -> Map<Self, OP>
        where OP: Fn(Self::Item) -> R, Self: Sized
    {
        Map { base: self,  map_op: op }
    }

    fn collect_into(self, target: &mut Vec<Self::Item>)
        where Self: Sized, Self::State: Send
    {
        collect_into(self, target);
    }
}

pub trait ParallelIteratorState: Sized {
    type Item;
    type Shared: Sync;

    fn len(&mut self) -> ParallelLen;

    fn split_at(self, index: usize) -> (Self, Self);

    fn for_each<OP>(self, shared: &Self::Shared, op: OP)
        where OP: FnMut(Self::Item);

    /// Initializes some number of consecutive entries at
    /// `target`. Returns number that were initialized.
    unsafe fn initialize(self, shared: &Self::Shared, mut target: *mut Self::Item) -> usize {
        let mut counter = 0;
        self.for_each(shared, |item| {
            ptr::write(target, item);
            target = target.offset(1);
            counter += 1;
        });
        counter
    }
}

#[derive(Copy, Clone)]
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

// The threshold cost where it is worth falling back to sequential.
pub const THRESHOLD: f64 = 10. * 1024.0;

///////////////////////////////////////////////////////////////////////////
//
//trait Dimension {
//    type Index;
//    type Item;
//}
//
//struct OneDimensional<T> {
//    phantom: PhantomData<*mut T>
//}
//
//impl<T> OneDimensional<T> {
//    pub fn new() -> OneDimensional<T> {
//        OneDimensional { phanton: PhantomData }
//    }
//}
//
//impl<T> Dimension for OneDimensional<T> {
//    type Index = usize;
//    type Item = T;
//}
//
//struct MoreDimensional<D: Dimension> {
//    phantom: PhantomData<*mut D>
//}
//
//impl<T> MoreDimensional<T> {
//    pub fn new() -> MoreDimensional<T> {
//        MoreDimensional { phanton: PhantomData }
//    }
//}
//
/////////////////////////////////////////////////////////////////////////////
//
//pub struct SliceIterator<'data, T> {
//    slice: &'data [T]
//}
//
//impl<'data, T> ParallelIterator for SliceIterator<'data, T> {
//    type Dim = OneDimensional<T>;
//}

///////////////////////////////////////////////////////////////////////////

pub struct SliceIter<'map, T: 'map> {
    slice: &'map [T]
}

impl<'map, T> IntoParallelIterator for &'map [T] {
    type Item = &'map T;
    type Iter = SliceIter<'map, T>;

    fn into_par_iter(self) -> Self::Iter {
        SliceIter { slice: self }
    }
}

impl<'map, T> ParallelIterator for SliceIter<'map, T> {
    type Item = &'map T;
    type Shared = ();
    type State = Self;

    fn state(self) -> (Self::Shared, Self::State) {
        ((), self)
    }
}

impl<'map, T> ParallelIteratorState for SliceIter<'map, T> {
    type Item = &'map T;
    type Shared = ();

    fn len(&mut self) -> ParallelLen {
        ParallelLen {
            maximal_len: self.slice.len(),
            cost: self.slice.len() as f64,
            sparse: false,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at(index);
        (left.into_par_iter(), right.into_par_iter())
    }

    fn for_each<OP>(self, _shared: &Self::Shared, mut op: OP)
        where OP: FnMut(&'map T)
    {
        for item in self.slice {
            op(item);
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct Map<M, MAP_OP> {
    base: M,
    map_op: MAP_OP,
}

impl<M, MAP_OP, R> ParallelIterator for Map<M, MAP_OP>
    where M: ParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync
{
    type Item = R;
    type Shared = MapShared<M, MAP_OP>;
    type State = MapState<M, MAP_OP>;

    fn state(self) -> (Self::Shared, Self::State) {
        let (base_shared, base_state) = self.base.state();
        (MapShared { base: base_shared, map_op: self.map_op },
         MapState { base: base_state, map_op: PhantomData })
    }
}

pub struct MapShared<M, MAP_OP>
    where M: ParallelIterator
{
    base: M::Shared,
    map_op: MAP_OP
}

pub struct MapState<M, MAP_OP>
    where M: ParallelIterator
{
    base: M::State,
    map_op: PhantomData<MAP_OP>
}

impl<M, MAP_OP, R> ParallelIteratorState for MapState<M, MAP_OP>
    where M: ParallelIterator,
          MAP_OP: Fn(M::Item) -> R + Sync
{
    type Item = R;
    type Shared = MapShared<M, MAP_OP>;

    fn len(&mut self) -> ParallelLen {
        self.base.len()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (MapState { base: left, map_op: self.map_op },
         MapState { base: right, map_op: self.map_op })
    }

    fn for_each<F>(self, shared: &Self::Shared, mut op: F)
        where F: FnMut(R)
    {
        self.base.for_each(&shared.base, |item| {
            op((shared.map_op)(item));
        });
    }
}

///////////////////////////////////////////////////////////////////////////

pub fn collect_into<PAR_ITER,T>(pi: PAR_ITER, v: &mut Vec<T>)
    where PAR_ITER: ParallelIterator<Item=T>, PAR_ITER::State: Send
{
    let (shared, mut state) = pi.state();
    let len = state.len();

    v.truncate(0); // clear any old data
    v.reserve(len.maximal_len); // reserve enough space
    let target = v.as_mut_ptr(); // get a raw ptr

    unsafe {
        collect_into_helper_with_len(state, &shared, len, CollectTarget(target));
    }

    unsafe {
        // TODO -- drops are not quite right here!
        v.set_len(len.maximal_len);
    }
}

unsafe fn collect_into_helper<STATE,T>(mut state: STATE,
                                       shared: &STATE::Shared,
                                       target: CollectTarget<T>)
    where STATE: ParallelIteratorState<Item=T> + Send
{
    let len = state.len();
    collect_into_helper_with_len(state, shared, len, target)
}

unsafe fn collect_into_helper_with_len<STATE,T>(state: STATE,
                                                shared: &STATE::Shared,
                                                len: ParallelLen,
                                                target: CollectTarget<T>)
    where STATE: ParallelIteratorState<Item=T> + Send
{
    if len.cost > THRESHOLD && len.maximal_len > 1 {
        let mid = len.maximal_len / 2;
        let (left, right) = state.split_at(mid);
        let (left_target, right_target) = target.split_at(mid);
        join(|| collect_into_helper(left, shared, left_target),
             || collect_into_helper(right, shared, right_target));
    } else {
        let initialized = state.initialize(&shared, target.as_mut_ptr());
        assert_eq!(initialized, len.maximal_len);
    }
}

struct CollectTarget<T>(*mut T);

unsafe impl<T> Send for CollectTarget<T> { }

impl<T> CollectTarget<T> {
    unsafe fn split_at(self, mid: usize) -> (CollectTarget<T>, CollectTarget<T>) {
        assert!(mid < (isize::MAX) as usize);
        let mid = mid as isize;
        (CollectTarget(self.0), CollectTarget(self.0.offset(mid)))
    }

    fn as_mut_ptr(self) -> *mut T {
        self.0
    }
}

///////////////////////////////////////////////////////////////////////////
//
//pub fn reduce<PAR_ITER,OP,T>(pi: PAR_ITER, op: OP) -> T
//    where PAR_ITER: ParallelIterator<Item=T>,
//          PAR_ITER::State: Send,
//          OP: Fn(T, T) -> T,
//{
//    let mut state = pi.into_state();
//    let len = state.len();
//    reduce_helper_with_len(state, len)
//}
//
//unsafe fn reduce_helper_with_len<STATE,T>(state: STATE,
//                                          len: ParallelLen)
//    where STATE: ParallelIteratorState<Item=T> + Send
//{
//    if len.cost > THRESHOLD && len.maximal_len > 1 {
//        let mid = len.maximal_len / 2;
//        let (left, right) = state.split_at(mid);
//        let (left_target, right_target) = target.split_at(mid);
//        join(|| collect_into_helper(left, left_target),
//             || collect_into_helper(right, right_target));
//    } else {
//        let initialized = state.initialize(target.as_mut_ptr());
//        assert_eq!(initialized, len.maximal_len);
//    }
//}
