use job::{Job, NULL_JOB};
use latch::Latch;
#[allow(unused_imports)]
use log::Event::*;
use rand;
use std::cell::Cell;
use std::sync::{Condvar, Mutex, Once, ONCE_INIT};
use std::thread;
use std::u32;
use util::leak;

///////////////////////////////////////////////////////////////////////////

const NUM_CPUS: usize = 1;

pub struct Registry {
    thread_infos: Vec<ThreadInfo>,
    state: Mutex<RegistryState>,
    work_available: Condvar,
}

struct RegistryState {
    threads_at_work: usize,
    injected_jobs: Vec<*mut Job>,
}

///////////////////////////////////////////////////////////////////////////
// Initialization

static mut THE_REGISTRY: Option<&'static Registry> = None;
static THE_REGISTRY_SET: Once = ONCE_INIT;
static THE_REGISTRY_INIT: Once = ONCE_INIT;

fn get_registry_or_init() -> &'static Registry {
    THE_REGISTRY_SET.call_once(|| {
        let registry = leak(Box::new(Registry::new()));
        unsafe { THE_REGISTRY = Some(registry); }
    });
    unsafe { THE_REGISTRY.unwrap() }
}

#[inline]
pub fn initialize() -> &'static Registry {
    THE_REGISTRY_INIT.call_once(|| {
        get_registry_or_init().finish_initializing();
    });
    unsafe { THE_REGISTRY.unwrap() }
}

impl Registry {
    fn new() -> Registry {
        for index in 0 .. NUM_CPUS {
            thread::spawn(move || unsafe { main_loop(index) });
        }

        Registry {
            thread_infos: (0..NUM_CPUS).map(|_| ThreadInfo::new()).collect(),
            state: Mutex::new(RegistryState::new()),
            work_available: Condvar::new(),
        }
    }

    fn finish_initializing(&self) {
        for info in &self.thread_infos {
            info.primed.wait();
        }
    }

    ///////////////////////////////////////////////////////////////////////////
    // MAIN LOOP
    //
    // So long as all of the worker threads are hanging out in their
    // top-level loop, there is no work to be done.

    fn start_working(&self, _index: usize) {
        log!(StartWorking { index: _index });
        let mut state = self.state.lock().unwrap();
        state.threads_at_work += 1;
        self.work_available.notify_all();
    }

    fn inject(&self, injected_jobs: &[*mut Job]) {
        log!(InjectJobs { count: injected_jobs.len() });
        let mut state = self.state.lock().unwrap();
        state.injected_jobs.extend(injected_jobs);
        self.work_available.notify_all();
    }

    fn wait_for_work(&self, _worker: usize, was_active: bool) -> Option<*mut Job> {
        log!(WaitForWork { worker: _worker, was_active: was_active });

        let mut state = self.state.lock().unwrap();

        if was_active {
            state.threads_at_work -= 1;
        }

        loop {
            // Otherwise, if anything was injected from outside,
            // return that.  Note that this gives preference to
            // injected items over stealing from others, which is a
            // bit dubious, but then so is the opposite.
            if let Some(job) = state.injected_jobs.pop() {
                state.threads_at_work += 1;
                self.work_available.notify_all();
                return Some(job);
            }

            // If any of the threads are running a job, we should spin
            // up, since they may generate subworkitems.
            if state.threads_at_work > 0 {
                return None;
            }

            state = self.work_available.wait(state).unwrap();
        }
    }
}

impl RegistryState {
    pub fn new() -> RegistryState {
        RegistryState {
            threads_at_work: 0,
            injected_jobs: Vec::new(),
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// WorkerThread identifiers

#[derive(Copy, Clone, Debug)]
pub struct WorkerThread(u32);

thread_local! {
    static WORKER_THREAD_INDEX: Cell<u32> = Cell::new(u32::MAX)
}

impl WorkerThread {
    /// Gets the `WorkerThread` index for the current thread, if any.
    #[inline]
    pub fn current() -> Option<WorkerThread> {
        let n = WORKER_THREAD_INDEX.with(|t| t.get());
        if n == u32::MAX { None } else { Some(WorkerThread(n)) }
    }

    /// Sets `self` as the worker thread index for the current thread.
    /// This is done during worker thread startup.
    fn set_current(&self) {
        assert!(self.0 != u32::MAX);
        assert!(WORKER_THREAD_INDEX.with(|t| t.get()) == u32::MAX);
        assert!(unsafe { THE_REGISTRY.is_some() });
        WORKER_THREAD_INDEX.with(|t| t.set(self.0));
    }

    #[inline]
    fn registry(&self) -> &'static Registry {
        // if you have gotten possession of a worker thread,
        // the registry must have been initialized by now
        unsafe { THE_REGISTRY.unwrap() }
    }

    #[inline]
    fn thread_info(&self) -> &'static ThreadInfo {
        &self.registry().thread_infos[self.0 as usize]
    }

    #[inline]
    pub unsafe fn push(&self, job: *mut Job) {
        let thread_info = self.thread_info();
        let mut deque = thread_info.deque.lock().unwrap();

        let top = deque.top;
        (*job).previous = top;
        if !top.is_null() {
            (*top).next = job;
        }

        deque.top = job;
        if deque.bottom.is_null() {
            deque.bottom = job;
        }
    }

    /// Pop `job` if it is still at the top of the stack.  Otherwise,
    /// some other thread has stolen this job.
    #[inline]
    pub unsafe fn pop(&self, job: *mut Job) -> bool {
        let thread_info = self.thread_info();
        let mut deque = thread_info.deque.lock().unwrap();
        if deque.top == job {
            deque.top = (*job).previous;
            if deque.bottom == job {
                deque.bottom = NULL_JOB;
            }
            true
        } else {
            false
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// WorkerThread info

struct ThreadInfo {
    // latch is set once thread has started and we are entering into
    // the main loop
    primed: Latch,
    deque: Mutex<ThreadDeque>,
}

struct ThreadDeque {
    bottom: *mut Job,
    top: *mut Job,
}

impl ThreadInfo {
    fn new() -> ThreadInfo {
        ThreadInfo {
            deque: Mutex::new(ThreadDeque::new()),
            primed: Latch::new(),
        }
    }
}

impl ThreadDeque {
    fn new() -> ThreadDeque {
        ThreadDeque { bottom: NULL_JOB, top: NULL_JOB }
    }
}

///////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(index: usize) {
    assert!(index < (u32::MAX as usize));
    let worker_index = WorkerThread(index as u32);

    // implicitly blocks until Registry is initialized, which means
    // that later, if we see that `WORKER_THREAD_INDEX` is valid, we know
    // that `THE_REGISTRY` is `Some(_)`
    let registry = get_registry_or_init();

    // store the thread index so we know that we are on a worker thread
    worker_index.set_current();

    // let registry know we are ready to do work
    registry.thread_infos[index].primed.set();

    let mut was_active = false;
    loop {
        if let Some(injected_job) = registry.wait_for_work(index, was_active) {
            (*injected_job).execute();
            was_active = true;
        } else if let Some(stolen_job) = steal_work(registry, index) {
            log!(StoleWork { worker: index, job: stolen_job });
            registry.start_working(index);
            (*stolen_job).execute();
            was_active = true;
        } else {
            was_active = false;
        }
    }
}

unsafe fn steal_work(registry: &Registry, index: usize) -> Option<*mut Job> {
    let start = rand::random::<usize>() % NUM_CPUS;
    (start .. NUM_CPUS)
        .chain(0 .. start)
        .filter(|&i| i != index)
        .filter_map(|i| steal_work_from(registry, i))
        .next()
}

unsafe fn steal_work_from(registry: &Registry, index: usize) -> Option<*mut Job> {
    let thread_info = &registry.thread_infos[index];
    let mut deque = thread_info.deque.lock().unwrap();
    if deque.bottom.is_null() {
        return None;
    }

    let job = deque.bottom;
    let next = (*job).next;
    deque.bottom = next;
    (*next).previous = NULL_JOB;
    Some(job)
}

pub fn inject(jobs: &[*mut Job]) {
    debug_assert!(WorkerThread::current().is_none());
    let registry = initialize();
    registry.inject(jobs);
}
