use std::marker::PhantomData;
use std::iter::IntoIterator;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

#[cfg(test)]
mod test;

/// A simple linked list, based on compare-and-swap. Values can only
/// be prepended to the front. At present, they can never be removed.
pub struct CasList<T> {
    head: AtomicPtr<Cell<T>>, // if not null, a unique, transmuted ptr to a Box<T>
}

struct Cell<T> {
    next: *mut Cell<T>, // if not null, a unique, transmuted ptr to a Box<T>
    data: T,
}

impl<T> CasList<T> {
    pub fn new() -> Self {
        CasList {
            head: AtomicPtr::new(ptr::null_mut())
        }
    }

    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        head.is_null()
    }

    pub fn prepend(&self, value: T) {
        unsafe {
            let cell = Box::new(Cell {
                next: ptr::null_mut(),
                data: value
            });

            // We're going to transfer ownership of this box into the
            // list. Note that none of this code below can panic,
            // which is important for avoiding leaks!
            let cell: *mut Cell<T> = mem::transmute(cell);
            loop {
                // FIXME -- order of operations wrong?
                // https://github.com/nikomatsakis/rayon/pull/141#discussion_r106792369
                let head = self.head.load(Ordering::Relaxed);
                (*cell).next = head;
                if self.head.compare_and_swap(head, cell, Ordering::Release) == head {
                    return;
                }
            }
        }
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            phantom: PhantomData,
            ptr: self.head.load(Ordering::Acquire),
        }
    }
}

impl<T> Drop for CasList<T> {
    fn drop(&mut self) {
        unsafe {
            let mut ptr: *mut Cell<T> = self.head.load(Ordering::Acquire);
            while !ptr.is_null() {
                let ptr_box: Box<Cell<T>> = mem::transmute(ptr);
                ptr = ptr_box.next;
                mem::drop(ptr_box);
            }
        }
    }
}

pub struct Iter<'iter, T: 'iter> {
    phantom: PhantomData<&'iter CasList<T>>,
    ptr: *mut Cell<T>
}

impl<'iter, T> Iterator for Iter<'iter, T> {
    type Item = &'iter T;

    fn next(&mut self) -> Option<&'iter T> {
        if self.ptr.is_null() {
            None
        } else {
            unsafe {
                let Cell { next, ref data } = *self.ptr;
                self.ptr = next;
                Some(data)
            }
        }
    }
}

impl<'iter, T> IntoIterator for &'iter CasList<T> {
    type Item = &'iter T;
    type IntoIter = Iter<'iter, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
