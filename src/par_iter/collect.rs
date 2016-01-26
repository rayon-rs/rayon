use api::join;
use super::ExactParallelIterator;
use super::len::*;
use super::state::*;
use std::isize;
use std::mem;
use std::ptr;

pub fn collect_into<PAR_ITER,T>(mut pi: PAR_ITER, v: &mut Vec<T>)
    where PAR_ITER: ExactParallelIterator<Item=T>,
          PAR_ITER: ExactParallelIterator,
          PAR_ITER::State: Send,
          T: Send,
{
    let len = pi.len();
    assert!(len < (isize::MAX) as usize);

    v.truncate(0); // clear any old data
    v.reserve(len); // reserve enough space
    let target = v.as_mut_ptr(); // get a raw ptr
    let consumer = CollectConsumer { target: target, len: len };
    pi.drive(consumer, &());

    unsafe {
        // TODO -- drops are not quite right here!
        v.set_len(len);
    }
}

unsafe fn collect_into_helper<STATE,T>(mut state: STATE,
                                       shared: &STATE::Shared,
                                       len: ParallelLen,
                                       target: CollectTarget<T>)
    where STATE: ParallelIteratorState<Item=T> + Send
{
    if len.cost > THRESHOLD && len.maximal_len > 1 {
        let mid = len.maximal_len / 2;
        let (left, right) = state.split_at(mid);
        let (left_target, right_target) = target.split_at(mid);
        join(|| collect_into_helper(left, shared, len.left_cost(mid), left_target),
             || collect_into_helper(right, shared, len.right_cost(mid), right_target));
    } else {
        let mut p = DropInitialized::new(target.as_mut_ptr());
        while let Some(item) = state.next(shared) {
            ptr::write(p.next, item);
            p.bump();
        }
        mem::forget(p); // fully initialized, so don't run the destructor
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

struct DropInitialized<T> {
    start: *mut T,
    next: *mut T,
}

impl<T> DropInitialized<T> {
    fn new(p: *mut T) -> DropInitialized<T> {
        DropInitialized { start: p, next: p }
    }

    unsafe fn bump(&mut self) {
        self.next = self.next.offset(1);
    }
}

impl<T> Drop for DropInitialized<T> {
    fn drop(&mut self) {
        unsafe {
            let mut p = self.start;
            while p != self.next {
                ptr::read(p);
                p = p.offset(1);
            }
        }
    }
}

struct CollectConsumer<ITEM: Send> {
    target: *mut ITEM,
    len: usize,
}

unsafe impl<ITEM: Send> Send for CollectConsumer<ITEM> { }

impl<'c, ITEM: Send + 'c> Consumer<'c> for CollectConsumer<ITEM> {
    type Item = ITEM;
    type Shared = ();
    type SeqState = DropInitialized<ITEM>;
    type Result = ();

    fn cost(&mut self, shared: &Self::Shared, cost: f64) -> f64 {
        cost * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, _: &Self::Shared, index: usize) -> (Self, Self) {
        assert!(index < self.len);
        (CollectConsumer { target: self.target, len: index },
         CollectConsumer { target: self.target.offset(index as isize), len: self.len - index })
    }

    unsafe fn start(&mut self, _reduce_op: &()) -> DropInitialized<ITEM> {
        DropInitialized::new(self.target)
    }

    unsafe fn consume(&mut self,
                      _: &(),
                      mut p: DropInitialized<ITEM>,
                      item: ITEM)
                      -> DropInitialized<ITEM> {
        ptr::write(p.next, item);
        p.bump();
        p
    }

    unsafe fn complete(self,
                       _: &(),
                       p: DropInitialized<ITEM>) {
        mem::forget(p); // fully initialized, so don't run the destructor
    }

    unsafe fn reduce(_: &(), a: (), b: ()) {
    }
}
