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
        let top = self.top.load(Ordering::SeqCst);   // Probably could be "Relaxed"
        self.jobs[top].store(job, Ordering::SeqCst); // Unclear if AtomicPtr is needed here?
        self.top.store(top + 1, Ordering::SeqCst);
    }

    #[inline]
    pub unsafe fn pop(&self) -> bool {
        let top = self.top.load(Ordering::SeqCst);
        let previous = top - 1;
        self.top.store(previous, Ordering::SeqCst);

        let bottom = self.bottom.load(Ordering::SeqCst);
        if bottom > previous {
            self.top.store(top, Ordering::SeqCst);
            let _lock = self.lock.lock().unwrap();
            let bottom = self.bottom.load(Ordering::SeqCst);
            if bottom > previous {
                return false;
            }
            self.top.store(previous, Ordering::SeqCst);
        }

        true
    }

    #[inline]
    pub unsafe fn steal(&self) -> Option<*mut Job> {
        let _lock = self.lock.lock().unwrap();

        let bottom = self.bottom.load(Ordering::SeqCst);
        let next = bottom + 1;
        self.bottom.store(next, Ordering::SeqCst);

        let top = self.top.load(Ordering::SeqCst);
        if next > top {
            self.bottom.store(bottom, Ordering::SeqCst);
            None
        } else {
            Some(self.jobs[bottom].load(Ordering::SeqCst))
        }
    }
}

