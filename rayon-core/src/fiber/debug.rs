use super::*;
use std::collections::HashMap;

pub type FiberInfoType = Arc<FiberInfo>;

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum FiberId {
    Worker(usize),
    Job(usize),
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum FiberStatus {
    WaitingOnLatch(Ptr),
    OnQueue(usize),
    New,
    Complete,
    Resumed,
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum FiberAction {
    Working,
    WaitUntil(Ptr),
}

#[derive(Debug)]
pub struct FiberInfo {
    pub id: FiberId,
    pub status: Mutex<FiberStatus>,
    pub action: Mutex<FiberAction>,
    pub stack_low: Mutex<Ptr>,
    pub stack_high: Mutex<Ptr>,
}

impl FiberInfo {
    pub fn new(id: FiberId, stack_low: usize, stack_high: usize) -> Self {
        FiberInfo {
            id,
            status: Mutex::new(FiberStatus::New),
            action: Mutex::new(FiberAction::Working),
            stack_low: Mutex::new(Ptr(stack_low)),
            stack_high: Mutex::new(Ptr(stack_high)),
        }
    }
}

#[derive(Debug)]
pub struct SingleWaiterLatch {
    pub data: Arc<Mutex<(bool, Option<(usize, Fiber)>, Ptr)>>,
}

impl SingleWaiterLatch {
    pub fn new() -> SingleWaiterLatch {
        let r = SingleWaiterLatch {
            data: Arc::new(Mutex::new((false, None, Ptr(0)))),
        };
        let id = &*r.data as *const _ as usize;
        fiber_log!("starting latch {:x}", id);
        #[cfg(feature = "debug")]
        LATCHES.lock().insert(id, r.data.clone());
        r
    }

    pub fn start(&self) {
        self.data.lock().2 = Ptr(self as *const _ as usize);
    }
}

impl Drop for SingleWaiterLatch {
    fn drop(&mut self) {
        let id = &*self.data as *const _ as usize;
        fiber_log!("dropping latch {:x}", id);
        #[cfg(feature = "debug")]
        LATCHES.lock().remove(&id);
    }
}

impl LatchProbe for SingleWaiterLatch {
    #[inline]
    fn probe(&self) -> bool {
        self.data.lock().0
    }
}

impl Latch for SingleWaiterLatch {
    fn set(&self) {
        let mut data = self.data.lock();
        let id = &*self.data as *const _ as usize;
        fiber_log!("setting latch {:x} {:?}", id, *data);
        debug_assert!(!data.0);
        data.0 = true;
        if let Some((worker_index, fiber)) = data.1.take() {
            fiber_log!("resuming fiber {} (worker {}) from latch {:x}", fiber, worker_index, self as *const _ as usize);
            let registry = Registry::current();
            registry.resume_fiber(worker_index, fiber);
        }
        // FIXME: Find out why this is needed. Rayon tickles all jobs after they are complete.
        // Perhaps this is unsufficient when jobs can wait multiple times?
        // Perhaps a job sets this latch then waits on something else?
        // registry.sleep.tickle(usize::MAX);
    }
}

impl Waitable for SingleWaiterLatch {
    fn complete(&self, _worker_thread: &WorkerThread) -> bool {
        self.probe()
    }

    fn await(&self, worker_thread: &WorkerThread, waiter: Fiber) {
        let mut data = self.data.lock();
        if data.0 {
            #[cfg(feature = "debug")]
            {
                *waiter.1.status.lock() = FiberStatus::OnQueue(worker_thread.index());
            }
            fiber_log!("resuming fiber {} (worker {}) because it tried to await on a complete latch {:x}", waiter, worker_thread.index(), self as *const _ as usize);
            worker_thread.registry.resume_fiber(worker_thread.index(), waiter);
        } else {
            #[cfg(feature = "debug")]
            {
                *waiter.1.status.lock() = FiberStatus::WaitingOnLatch(Ptr(self as *const _ as _));
            }
            fiber_log!("fiber {} (worker {}) is waiting on latch {:x}", waiter, worker_thread.index(), self as *const _ as usize);
            data.1 = Some((worker_thread.index(), waiter));
        }
    }
}

#[derive(Debug)]
pub struct WaiterLatch {
    pub data: Arc<Mutex<(bool, Vec<(usize, Fiber)>, Ptr)>>,
}

impl WaiterLatch {
    pub fn new() -> WaiterLatch {
        let r = WaiterLatch {
            data: Arc::new(Mutex::new((false, Vec::new(), Ptr(0)))),
        };
        let id = &*r.data as *const _ as usize;
        fiber_log!("starting latch {:x}", id);
        #[cfg(feature = "debug")]
        WAITER_LATCHES.lock().insert(id, r.data.clone());
        r
    }

    pub fn start(&self) {
        self.data.lock().2 = Ptr(self as *const _ as usize);
    }
}

impl Drop for WaiterLatch {
    fn drop(&mut self) {
        let id = &*self.data as *const _ as usize;
        fiber_log!("dropping latch {:x}", id);
        #[cfg(feature = "debug")]
        WAITER_LATCHES.lock().remove(&id);
    }
}

impl LatchProbe for WaiterLatch {
    #[inline]
    fn probe(&self) -> bool {
        self.data.lock().0
    }
}

impl Latch for WaiterLatch {
    fn set(&self) {
        let mut data = self.data.lock();
        debug_assert!(!data.0);
        data.0 = true;
        for (worker_index, fiber) in data.1.drain(..) {
            fiber_log!("resuming fiber {} (worker {}) from latch {:x}", fiber, worker_index, self as *const _ as usize);
            let registry = Registry::current();
            registry.resume_fiber(worker_index, fiber);
        }
    }
}

impl Waitable for WaiterLatch {
    fn complete(&self, _worker_thread: &WorkerThread) -> bool {
        self.probe()
    }

    fn await(&self, worker_thread: &WorkerThread, waiter: Fiber) {
        let mut data = self.data.lock();
        if data.0 {
            #[cfg(feature = "debug")]
            {
                *waiter.1.status.lock() = FiberStatus::OnQueue(worker_thread.index());
            }
            fiber_log!("resuming fiber {} (worker {}) because it tried to await on a complete latch {:x}", waiter, worker_thread.index(), self as *const _ as usize);
            worker_thread.registry.resume_fiber(worker_thread.index(), waiter);
        } else {
            #[cfg(feature = "debug")]
            {
                *waiter.1.status.lock() = FiberStatus::WaitingOnLatch(Ptr(self as *const _ as _));
            }
            fiber_log!("fiber {} (worker {}) is waiting on latch {:x}", waiter, worker_thread.index(), self as *const _ as usize);
            data.1.push((worker_thread.index(), waiter));
        }
    }
}

lazy_static! {
    pub static ref FIBERS: Mutex<HashMap<FiberId, Arc<FiberInfo>>> = {
        Mutex::new(HashMap::new())
    };
    pub static ref LATCHES: Mutex<HashMap<usize, Arc<Mutex<(bool, Option<(usize, Fiber)>, Ptr)>>>> = {
        Mutex::new(HashMap::new())
    };
    pub static ref COUNT_LATCHES: Mutex<HashMap<usize, Arc<Mutex<(usize, Option<(usize, Fiber)>)>>>> = {
        Mutex::new(HashMap::new())
    };
    pub static ref WAITER_LATCHES: Mutex<HashMap<usize, Arc<Mutex<(bool, Vec<(usize, Fiber)>, Ptr)>>>> = {
        Mutex::new(HashMap::new())
    };
}

pub fn ctrlc() {
    debug_log!("------ fibers ------");
    for (&id, data) in &*FIBERS.lock() {
        debug_log!("fiber {:?}", id);
        debug_log!("   status {:?}", *data.status.lock());
        debug_log!("   action: {:?}", *data.action.lock());
        debug_log!("   stack_low: {:?}", *data.stack_low.lock());
        debug_log!("   stack_high: {:?}", *data.stack_high.lock());
    }
    debug_log!("------ end fibers ------");
    debug_log!("------ latches ------");
    for (&id, data) in &*LATCHES.lock() {
        debug_log!("latch {:x}, info {:?}", id, *data);
    }
    for (&id, data) in &*WAITER_LATCHES.lock() {
        debug_log!("waiter latch {:x}, info {:?}", id, *data);
    }
    for (&id, data) in &*COUNT_LATCHES.lock() {
        debug_log!("count latch {:x}, info {:?}", id, *data);
    }
    for (&id, data) in &*LATCHES.lock() {
        debug_log!("deref latch {:x}", id);
        unsafe { ptr::read_volatile((data.lock().2).0 as *const bool); }
    }
    debug_log!("------ end latches ------");
}
