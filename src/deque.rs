use job::Job;
use std::default::Default;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Mutex;

const INITIAL_SIZE: usize = 1024;

pub struct ThreadDeque {
    jobs: Vec<AtomicPtr<Job>>,
    bottom: AtomicUsize,
    top: AtomicUsize,
    lock: Mutex<()>,
}

impl ThreadDeque {
    pub fn new() -> ThreadDeque {
        ThreadDeque {
            jobs: (0..INITIAL_SIZE).map(|_| AtomicPtr::default()).collect(),
            bottom: AtomicUsize::default(),
            top: AtomicUsize::default(),
            lock: Mutex::new(())
        }
    }

    #[inline]
    pub unsafe fn push(&self, job: *mut Job) {
        let top = self.top.load(Ordering::Relaxed); // (1)
        self.jobs[top].store(job, Ordering::Relaxed); // (4)
        self.top.store(top + 1, Ordering::SeqCst);
    }

    #[inline]
    pub unsafe fn pop(&self) -> bool {
        let top = self.top.load(Ordering::Relaxed); // (1)
        let previous = top - 1;
        self.top.store(previous, Ordering::SeqCst);

        let bottom = self.bottom.load(Ordering::SeqCst);
        if bottom > previous {
            self.top.store(top, Ordering::SeqCst);
            let _lock = self.lock.lock().unwrap();
            let bottom = self.bottom.load(Ordering::Relaxed); // (2)
            if bottom > previous {
                return false;
            }
            self.top.store(previous, Ordering::Relaxed); // (1, 2)
        }

        true
    }

    #[inline]
    pub unsafe fn steal(&self) -> Option<*mut Job> {
        let _lock = self.lock.lock().unwrap();

        let bottom = self.bottom.load(Ordering::Relaxed); // (2)
        let next = bottom + 1;
        self.bottom.store(next, Ordering::SeqCst); // (3)

        let top = self.top.load(Ordering::SeqCst);
        if next > top {
            self.bottom.store(bottom, Ordering::SeqCst); // (3)
            None
        } else {
            Some(self.jobs[bottom].load(Ordering::Relaxed)) // (4)
        }
    }
}

// Justification and notes regarding memory ordering:
// (1) The top variable is only modified by the owner thread, so *reads* need not
//     synchronize with other threads.
// (2) The bottom variable is only *mutated* under lock, so once we've acquired
//     the lock we can read with a relaxed ordering.
// (3) Nonetheless, bottom is read by owner thread without lock, so stores
//     require some fences.
// (4) The jobs array itself is effectively guarded by reads and
//     stores to `top`, so relaxed ordering is sufficient.
