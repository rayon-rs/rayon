use super::ExactParallelIterator;
use super::len::*;
use super::internal::*;
use std::isize;
use std::mem;
use std::ptr;

pub fn collect_into<PAR_ITER,T>(mut pi: PAR_ITER, v: &mut Vec<T>)
    where PAR_ITER: ExactParallelIterator<Item=T>,
          PAR_ITER: ExactParallelIterator,
          T: Send,
{
    let len = pi.len();
    assert!(len < (isize::MAX) as usize);

    v.truncate(0); // clear any old data
    v.reserve(len); // reserve enough space
    let target = v.as_mut_ptr(); // get a raw ptr
    let consumer = CollectConsumer { target: target, len: len };
    pi.drive(consumer);

    unsafe {
        // TODO -- drops are not quite right here!
        v.set_len(len);
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

impl<ITEM: Send> Consumer for CollectConsumer<ITEM> {
    type Item = ITEM;
    type SeqState = DropInitialized<ITEM>;
    type Result = ();

    fn cost(&mut self, cost: f64) -> f64 {
        cost * FUNC_ADJUSTMENT
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        assert!(index < self.len);
        (CollectConsumer { target: self.target, len: index },
         CollectConsumer { target: self.target.offset(index as isize), len: self.len - index })
    }

    unsafe fn start(&mut self) -> DropInitialized<ITEM> {
        DropInitialized::new(self.target)
    }

    unsafe fn consume(&mut self,
                      mut p: DropInitialized<ITEM>,
                      item: ITEM)
                      -> DropInitialized<ITEM> {
        ptr::write(p.next, item);
        p.bump();
        p
    }

    unsafe fn complete(self,
                       p: DropInitialized<ITEM>) {
        mem::forget(p); // fully initialized, so don't run the destructor
    }

    unsafe fn reduce(_: (), _: ()) {
    }
}
