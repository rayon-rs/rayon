use registry::{self, WorkerThread};
#[cfg(feature = "tlv")]
use fiber::tlv;
use fiber::SingleWaiterLatch;
use job::{Job, JobRef};
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use std::iter;
use latch::Latch;

#[repr(align(64))]
#[derive(Debug)]
struct CacheAligned<T>(T);
/*
#[derive(Debug)]
struct Slice {
    start: usize,
    len: AtomicIsize, // Limit this to 31 bits to allow underflow when stealing
}

impl Slice {
    fn new(start: usize, len: usize) -> Self {
        Slice {
            start,
            len: AtomicIsize::new(len as isize),
        }
    }

    fn take(&self) -> Option<usize> {
        loop {
            let old_len = self.len.fetch_sub(1, Ordering::SeqCst);
            if old_len <= 0 {
                self.len.fetch_add(1, Ordering::SeqCst);
                return None;
            }
            return Some(self.start + (old_len as usize) - 1);
        }
    }

    fn take2(&self) -> Option<usize> {
        loop {
            let current = self.len.load(Ordering::SeqCst);
            if current == 0 {
                return None;
            }
            let result = self.len.compare_exchange_weak(current,
                                                        current - 1,
                                                        Ordering::SeqCst,
                                                        Ordering::SeqCst);
            if result.is_ok() {
                return Some(self.start + (current as usize) - 1);
            }
        }
    }

    fn steal(&self) -> Option<usize> {
        loop {
            let current = self.len.load(Ordering::SeqCst);
            if current == 0 {
                return None;
            }
            let result = self.len.compare_exchange_weak(current,
                                                        current - 1,
                                                        Ordering::SeqCst,
                                                        Ordering::SeqCst);
            if result.is_ok() {
                return Some(self.start + (current as usize) - 1);
            }
        }
    }
}*/

#[derive(Debug)]
struct Slice {
    data: AtomicUsize,
}

impl Slice {
    #[inline(always)]
    fn new(start: usize, len: usize) -> Self {
        Slice {
            data: AtomicUsize::new(Slice::encode(start, len)),
        }
    }

    #[inline(always)]
    fn encode(start: usize, len: usize) -> usize {
        (len as u32 as usize) | ((start as u32 as usize) << 32)
    }

    #[inline(always)]
    fn decode(data: usize) -> (usize, usize) {
        (data >> 32, data as u32 as usize)
    }

    #[inline]
    fn take_one(self) -> (usize, Slice) {
        let (start, len) = Slice::decode(self.data.into_inner());
        (start + len - 1, Slice { data: AtomicUsize::new(Slice::encode(start, len - 1)) })
    }

    #[inline]
    fn take(&self) -> Option<usize> {
        loop {
            let data = self.data.load(Ordering::SeqCst);
            let (old_start, old_len) = Slice::decode(data);
            if old_len == 0 {
                return None;
            }
            let result = self.data.compare_exchange_weak(data,
                                                         Slice::encode(old_start, old_len - 1),
                                                         Ordering::SeqCst,
                                                         Ordering::SeqCst);
            if result.is_ok() {
                return Some(old_start + old_len - 1);
            }
        }
    }

    #[inline]
    fn steal_half(&self) -> Option<Slice> {
        loop {
            let data = self.data.load(Ordering::SeqCst);
            let (start, mut len) = Slice::decode(data);
            len = match len {
                0 => return None,
                1 => 1,
                _ => len / 2,
            };
            let result = self.data.compare_exchange_weak(data,
                                                         Slice::encode(start, len),
                                                         Ordering::SeqCst,
                                                         Ordering::SeqCst);
            if result.is_ok() {
                return Some(Slice { data: AtomicUsize::new(data) });
            }
        }
    }

    #[inline]
    fn replace(&self, new: Slice) {
        let new_data = new.data.into_inner();
        loop {
            let data = self.data.load(Ordering::SeqCst);
            assert!(Slice::decode(data).1 == 0);
            let result = self.data.compare_exchange_weak(data,
                                                         new_data,
                                                         Ordering::SeqCst,
                                                         Ordering::SeqCst);
            if result.is_ok() {
                return;
            }
        }
    }
}

struct ForkData<F: Fn(usize) + Send> {
    ref_count: CacheAligned<AtomicUsize>,
    active: CacheAligned<AtomicUsize>,
    slices: Vec<CacheAligned<Slice>>,
    f: F,
    complete: SingleWaiterLatch,
    #[cfg(feature = "tlv")]
    tlv: usize,
}

impl<F: Fn(usize) + Send> ForkData<F> {
    pub unsafe fn as_job_ref(this: *const Self) -> JobRef {
        JobRef::new(this)
    }

    unsafe fn decr(&self) {
        if self.ref_count.0.fetch_sub(1, Ordering::SeqCst) == 0 {
            // Destroy the data if we are the last reference
            Box::from_raw(self as *const Self as *mut Self);
        }
    }

    fn steal(&self, worker: &WorkerThread) -> Option<usize> {
        let worker_idx = worker.index();
        if let Some(v) = self.slices[worker_idx].0.take() {
            return Some(v);
        }

        for (i, slice) in self.slices.iter().enumerate() {
            if i == worker_idx {
                continue;
            }

            if let Some(s) = slice.0.take() {
                /*let (r, s) = s.take_one();
                self.slices[worker_idx].0.replace(s);*/
                return Some(s);
            }
        }

        None
    }

    fn iter(&self, i: usize) {
        //println!("process {}", i);
        (self.f)(i);
    }

    fn process(&self, worker: &WorkerThread, first: usize) {
        let _s = self.active.0.fetch_add(1, Ordering::SeqCst);
        //println!("worker start {}", _s);

        self.iter(first);

        while let Some(i) = self.steal(worker) {
            self.iter(i);
        }
        let r = self.active.0.fetch_sub(1, Ordering::SeqCst);
        //println!("worker done {}", r);
        if r == 1 {
            // There was no more work to steal and we are the last active worker
            //println!("set!");
            self.complete.set();
        }
    }
}

impl<F: Fn(usize) + Send> Job for ForkData<F> {
    unsafe fn execute(this: *const Self) {
        let this = &*this;
        #[cfg(feature = "tlv")]
        tlv::TLV.with(|tlv| tlv.set(this.tlv));

        let worker = &*WorkerThread::current();
    
        if let Some(first) = this.steal(worker) {
            //println!("started worker");
            worker.push(ForkData::as_job_ref(this));

            this.process(worker, first);
        } else {
            //println!("dummy");
            this.decr();
        }
    }
}

pub fn fork<F: Fn(usize) + Send>(n: usize, f: F)
{
    registry::in_worker(|worker_thread, injected| unsafe {
        // FIXME: Fallback to regular Rayon iterators when `n` is too large
        let threads = worker_thread.registry.num_threads();

        let i_per_thread = n / threads;
        let last_thread = i_per_thread + n % threads;

        let data = Box::into_raw(Box::new(ForkData {
            ref_count: CacheAligned(AtomicUsize::new(threads + 1)),
            active: CacheAligned(AtomicUsize::new(0)),
            slices: iter::once(CacheAligned(Slice::new(0, last_thread)))
              .chain((0..(threads - 1)).map(|i| {
                CacheAligned(Slice::new(last_thread + i * i_per_thread, i_per_thread))
            })).collect(),
            complete: SingleWaiterLatch::new(),
            f,
            #[cfg(feature = "tlv")]
            tlv: tlv::get(),
        }));

        //println!("slices: {:?}", (*data).slices);

        for _ in 0..threads {
            worker_thread.worker.push(ForkData::as_job_ref(data));
        }
        worker_thread.registry.signal();

        //(*data).process();

        // Wait for things to complete
        worker_thread.wait_until(&(*data).complete);

        //println!("fork end");

        (*data).decr();
    })
}