
#[cfg(not(windows))]
use context::{Context, Transfer};
#[cfg(not(windows))]
use context::stack::{ProtectedFixedSizeStack};
use std::mem::{self, ManuallyDrop};
use std::ptr;
use std::usize;
#[cfg(not(windows))]
use std::intrinsics;
use std::fmt;
#[cfg(feature = "debug")]
use std::fmt::Display;
use job::JobRef;
use registry::WorkerThread;
use latch::{Latch, LatchProbe};
use registry::Registry;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
#[cfg(windows)]
use kernel32;
#[cfg(windows)]
use winapi;
#[cfg(windows)]
use std::io;

#[cfg(feature = "debug")]
pub use self::debug::debug_print_state;

#[cfg(feature = "debug")]
pub use self::debug::{SingleWaiterLatch, WaiterLatch};
#[cfg(not(feature = "debug"))]
pub use self::normal::{SingleWaiterLatch, WaiterLatch};

#[cfg(feature = "debug")]
mod debug;

#[cfg(feature = "debug")]
pub use self::debug::{FiberStatus, FiberInfo, FiberId, FiberAction, COUNT_LATCHES, LATCHES, FIBERS};

#[cfg(feature = "tlv")]
pub mod tlv {
    use std::cell::Cell;

    thread_local!(pub(crate) static TLV: Cell<usize> = Cell::new(0));

    pub fn set<F: FnOnce() -> R, R>(value: usize, f: F) -> R {
        struct Reset(usize);
        impl Drop for Reset {
            fn drop(&mut self) {
                TLV.with(|tlv| tlv.set(self.0));
            }
        }
        let _reset = Reset(get());
        TLV.with(|tlv| tlv.set(value));
        f()
    }

    pub fn get() -> usize {
        TLV.with(|tlv| tlv.get())
    }

    #[test]
    fn tlv_is_preserved() {
        use fiber::WaiterLatch;
        use scope::scope;
        use registry;
        use latch::Latch;

        set(999, || {
            let latch = WaiterLatch::new();
            let tcx = get();
            registry::in_worker(|worker, _| {
                scope(|s| {
                    for i in 0..100 {
                        s.spawn(move |_| {  TLV.with(|tlv| tlv.set(i)); });
                    }
                    s.spawn(|_| { latch.set(); });
                    unsafe {
                        worker.wait_until(&latch);
                    }
                });
            });
            assert_eq!(tcx, get());
        });
    }
}

#[cfg(windows)]
extern {
    fn __rayon_core_get_current_fiber() -> usize;
}

// FIXME: Ensure uses of this do not leak
#[derive(Debug)]
pub struct UnsafeOption<T> {
    #[cfg(debug_assertions)]
    there: bool,
    val: ManuallyDrop<UnsafeCell<T>>,
}

impl<T> UnsafeOption<T> {
    pub fn none() -> Self {
        UnsafeOption {
            #[cfg(debug_assertions)]
            there: false,
            val: unsafe {
                ManuallyDrop::new(UnsafeCell::new(mem::uninitialized()))
            },
        }
    }

    pub fn some(t: T) -> Self {
        UnsafeOption {
            #[cfg(debug_assertions)]
            there: true,
            val: unsafe {
                ManuallyDrop::new(UnsafeCell::new(t))
            },
        }
    }

    pub fn take(&self) -> T {
        unsafe {
            #[cfg(debug_assertions)]
            {
                assert!(self.there);
                ptr::write(&self.there as *const _ as *mut _, false);
            }

            ptr::read(self.val.get())
        }
    }

    pub fn give(&self, t: T) {
        unsafe {
            #[cfg(debug_assertions)]
            {
                assert!(!self.there);
                ptr::write(&self.there as *const _ as *mut _, true);
            }

            ptr::write(self.val.get(), t);
        }
    }
}

unsafe impl<T> Send for UnsafeOption<T> {}
unsafe impl<T> Sync for UnsafeOption<T> {}

#[cfg(not(feature = "debug"))]
mod normal {
    use super::*;

    #[derive(Debug)]
    pub struct SingleWaiterLatch {
        // 0 - incomplete
        // 1 - incomplete with fiber
        // 2 - complete
        state: AtomicUsize,
        waiter: UnsafeOption<(usize, Fiber)>,
    }

    impl SingleWaiterLatch {
        pub fn new() -> SingleWaiterLatch {
            SingleWaiterLatch {
                state: AtomicUsize::new(0),
                waiter: UnsafeOption::none(),
            }
        }

        pub fn start(&self) {}
    }

    impl LatchProbe for SingleWaiterLatch {
        #[inline]
        fn probe(&self) -> bool {
            self.state.load(Ordering::SeqCst) == 2
        }
    }

    impl Latch for SingleWaiterLatch {
        fn set(&self) {
            loop {
                match self.state.load(Ordering::SeqCst) {
                    0 => {
                        let result = self.state.compare_exchange_weak(0,
                                                                    2,
                                                                    Ordering::SeqCst,
                                                                    Ordering::SeqCst);
                        if result.is_ok() {
                            break;
                        }
                    }
                    1 => {
                        self.state.store(2, Ordering::SeqCst);
                        let (worker_index, fiber) = self.waiter.take();
                        Registry::current().resume_fiber(worker_index, fiber);
                        break;
                    }
                    _ => panic!(),
                }
            }
            Registry::current().signal();
        }
    }

    impl Waitable for SingleWaiterLatch {
        fn complete(&self, _worker_thread: &WorkerThread) -> bool {
            self.probe()
        }

        fn await(&self, worker_thread: &WorkerThread, mut waiter: Fiber, _tlv: usize) {
            loop {
                match self.state.load(Ordering::SeqCst) {
                    0 => {
                        self.waiter.give((worker_thread.index(), waiter));
                        let result = self.state.compare_exchange(0,
                                                                1,
                                                                Ordering::SeqCst,
                                                                Ordering::SeqCst);
                        if result.is_err() {
                            waiter = self.waiter.take().1;
                            break;
                        }

                        return;
                    }
                    2 => break,
                    _ => panic!(),
                }
            }
            // The latch was already set, so just immediately add the waiter back to the work list
            worker_thread.registry.resume_fiber(worker_thread.index(), waiter);
        }
    }

    #[derive(Debug)]
    pub struct WaiterLatch {
        pub data: Mutex<(bool, Vec<(usize, Fiber)>)>,
    }

    impl WaiterLatch {
        pub fn new() -> WaiterLatch {
            WaiterLatch {
                data: Mutex::new((false, Vec::new())),
            }
        }
        pub fn start(&self) {}
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

        fn await(&self, worker_thread: &WorkerThread, waiter: Fiber, _tlv: usize) {
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
}

#[derive(Debug)]
pub struct SingleWaiterCountLatch {
    pub data: Arc<Mutex<(usize, Option<(usize, Fiber)>)>>,
}

impl SingleWaiterCountLatch {
    pub fn with_count(c: usize) -> SingleWaiterCountLatch {
        let r = SingleWaiterCountLatch {
            data: Arc::new(Mutex::new((c, None))),
        };
        let _id = &*r.data as *const _ as usize;
        fiber_log!("starting latch {:x}", _id);
        #[cfg(feature = "debug")]
        debug::COUNT_LATCHES.lock().insert(_id, r.data.clone());
        r
    }

    pub fn new() -> SingleWaiterCountLatch {
        Self::with_count(1)
    }

    pub fn increment(&self) {
        let mut data = self.data.lock();
        debug_assert!(data.0 > 0);
        data.0 += 1;
    }

    pub fn try_increment(&self) -> bool {
        let mut data = self.data.lock();
        if data.0 > 0 {
            data.0 += 1;
            true
        } else {
            false
        }
    }
}

impl Drop for SingleWaiterCountLatch {
    fn drop(&mut self) {
        let _id = &*self.data as *const _ as usize;
        fiber_log!("dropping count latch {:x}", _id);
        #[cfg(feature = "debug")]
        debug::COUNT_LATCHES.lock().remove(&_id);
    }
}

impl LatchProbe for SingleWaiterCountLatch {
    #[inline]
    fn probe(&self) -> bool {
        self.data.lock().0 == 0
    }
}

impl Latch for SingleWaiterCountLatch {
    fn set(&self) {
        let mut data = self.data.lock();
        debug_assert!(data.0 > 0);
        data.0 -= 1;
        if data.0 == 0 {
            if let Some((worker_index, fiber)) = data.1.take() {
                Registry::current().resume_fiber(worker_index, fiber);
            }
        }
        Registry::current().signal();
    }
}

impl Waitable for SingleWaiterCountLatch {
    fn complete(&self, _worker_thread: &WorkerThread) -> bool {
        self.probe()
    }

    fn await(&self, worker_thread: &WorkerThread, waiter: Fiber, _tlv: usize) {
        let mut data = self.data.lock();
        if data.0 > 0 {
            data.1 = Some((worker_thread.index(), waiter));
        } else {
            worker_thread.registry.resume_fiber(worker_thread.index(), waiter);
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Ptr(pub usize);

impl fmt::Display for Ptr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::Debug for Ptr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

#[cfg(feature = "debug")]
use self::debug::FiberInfoType;
#[cfg(not(feature = "debug"))]
type FiberInfoType = ();

#[cfg(not(windows))]
pub type FiberStack = ProtectedFixedSizeStack;
#[cfg(not(windows))]
type FiberContext = Context;

#[cfg(windows)]
pub type FiberStack = WinFiber;
#[cfg(windows)]
type FiberContext = usize;

#[must_use]
#[derive(Debug)]
pub struct Fiber(FiberContext, FiberInfoType);

#[cfg(feature = "debug")]
impl Display for Fiber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use fmt::Debug;
        self.1.id.fmt(f)
    }
}

#[cfg(feature = "debug")]
static FIBER_ID: AtomicUsize = AtomicUsize::new(1);

impl Drop for Fiber {
    fn drop(&mut self) {
        panic!("Dropped a fiber");
    }
}

pub trait Waitable {
    fn complete(&self, worker_thread: &WorkerThread) -> bool;
    fn await(&self, worker_thread: &WorkerThread, waiter: Fiber, tlv: usize);
}

pub enum ResumeAction {
    StoreInWaitable(*const Waitable, usize),
    FreeStack(FiberStack),
}

impl ResumeAction {
    fn run(self, worker_thread: &WorkerThread, fiber: Fiber) {
        match self {
            ResumeAction::StoreInWaitable(waitable, tlv) => {
                unsafe { (*waitable).await(worker_thread, fiber, tlv) }
            }
            ResumeAction::FreeStack(stack) => {
                #[cfg(feature = "debug")]
                {
                    stack.poison();
                    let mut list = worker_thread.poisoned_stacks.borrow_mut();
                    if list.len() > 0x100 {
                        fiber_log!("| too many stacks |");
                        list.pop_front();
                    }
                    fiber_log!("done using stack {:x} - {:x}", stack.bottom()as usize, stack.top() as usize);
                    list.push_back(stack);
                }

                #[cfg(not(feature = "debug"))]
                worker_thread.stack_cache.borrow_mut().push(stack);

                mem::forget(fiber);
            },
        }
    }
}

struct FiberData {
    stack: FiberStack,
    job: JobRef,
    action: ResumeAction,
    #[cfg(windows)]
    context: FiberContext,
}

fn run_fiber(data: FiberData, context: FiberContext, worker: &WorkerThread) -> TransferInfo {
    unsafe {
        #[cfg(feature = "debug")]
        worker.registry.active_fibers.fetch_add(1, Ordering::SeqCst);

        #[cfg(feature = "debug")]
        let _self_data = worker.current_fiber.borrow().0.clone().unwrap();
        #[cfg(feature = "debug")]
        let old_data = worker.current_fiber.borrow().1.clone().unwrap();
        #[cfg(not(feature = "debug"))]
        let old_data = ();

        #[cfg(feature = "debug")]
        #[cfg(windows)] {
            let mut stack_low = 0;
            let mut stack_high = 0;
            ::kernel32::GetCurrentThreadStackLimits(&mut stack_low, &mut stack_high);
            *_self_data.stack_low.lock() = Ptr(stack_low as usize);
            *_self_data.stack_high.lock() = Ptr(stack_high as usize);
        }

        #[cfg(feature = "debug")]
        FIBERS.lock().insert(_self_data.id, _self_data.clone());

        fiber_log!("fiber starting");
        data.action.run(worker, Fiber(context, old_data));
        data.job.execute();
        let stack = data.stack;

        #[cfg(feature = "debug")]
        FIBERS.lock().remove(&_self_data.id);

        fiber_log!("fiber complete");

        #[cfg(feature = "debug")]
        worker.registry.active_fibers.fetch_sub(1, Ordering::SeqCst);

        worker.find_work(ResumeAction::FreeStack(stack))
    }
}

#[cfg(not(windows))]
extern "C" fn context_function(t: Transfer) -> ! {
    unsafe {
        let worker = &*WorkerThread::current();
        run_fiber(ptr::read(t.data as *mut FiberData), t.context, worker);
        intrinsics::abort();
    }
}

#[cfg(windows)]
pub struct WinFiber(winapi::PVOID);

#[cfg(windows)]
impl WinFiber {
    #[cfg(feature = "debug")]
    fn top(&self) -> *const () { 0i32 as _ }
    #[cfg(feature = "debug")]
    fn bottom(&self) -> *const () { 0i32 as _ }
    #[cfg(feature = "debug")]
    fn poison(&self) { }

    fn new(stack_size: usize) -> Option<Self> {
        unsafe {
            let fiber = kernel32::CreateFiberEx(0x1000,
                stack_size as _,
                0,
                Some(fiber_proc),
                0i32 as _);
            if fiber == 0i32 as _ {
                panic!("unable to allocate fiber {}", io::Error::last_os_error());
            }
            Some(WinFiber(fiber))
        }
    }
}

#[cfg(windows)]
impl Drop for WinFiber {
    fn drop(&mut self) {
        unsafe {
            kernel32::DeleteFiber(self.0);
        }
    }
}

#[cfg(windows)]
unsafe extern "system" fn fiber_proc(_: winapi::LPVOID) {
    // Ensure that we have a guard page. the fiber API doesn't not guarantee this
    #[cfg(debug_assertions)] {
        let mut stack_low = 0;
        let mut stack_high = 0;
        kernel32::GetCurrentThreadStackLimits(&mut stack_low, &mut stack_high);
        let mut mbi = mem::zeroed();
        if kernel32::VirtualQuery(stack_low as _, &mut mbi, 0x1000) != 0 {
            assert!(mbi.State == winapi::MEM_RESERVE);
        }
    }

    let worker = &*WorkerThread::current();
    loop {
        let data = NEW_FIBER_DATA.with(|fiber_data| fiber_data.take());
        let data = ptr::read(data as *mut FiberData);
        let context = data.context;
        // No-one will resume us without creating a new fiber, forget the transfer information
        mem::forget(run_fiber(data, context, worker));
    }
}

#[cfg(windows)]
thread_local! {
    static RESUME_INFO: UnsafeOption<(*mut ResumeAction, FiberContext)> = UnsafeOption::none();
    static NEW_FIBER_DATA: UnsafeOption<*mut FiberData> = UnsafeOption::none();
}

pub struct TransferInfo {
    #[cfg(not(windows))]
    old_context: Context,
    #[cfg(not(windows))]
    pending_action: usize,
}

impl TransferInfo {
    pub fn handle(self, worker: &WorkerThread) {
        unsafe {
            #[cfg(not(windows))]
            let (pending_action, old_context) = (self.pending_action, ptr::read(&self.old_context));
            #[cfg(windows)]
            let (pending_action, old_context) = RESUME_INFO.with(|resume_info| resume_info.take());

            #[cfg(feature = "debug")]
            let data = worker.current_fiber.borrow().1.clone().unwrap();
            #[cfg(not(feature = "debug"))]
            let data = ();

            ptr::read(pending_action as *mut ResumeAction).run(worker, Fiber(old_context, data));
        }
        mem::forget(self);
    }
}

impl Drop for TransferInfo {
    fn drop(&mut self) {
        panic!("Dropped TransferInfo");
    }
}

impl Fiber {
    #[cfg(not(windows))]
    fn to_context(self) -> Context {
        let r = unsafe { ptr::read(&self.0) };
        mem::forget(self);
        r
    }

    pub fn resume(self, _worker: &WorkerThread, mut action: ResumeAction) -> TransferInfo {
        #[cfg(feature = "tlv")]
        let saved_tlv = tlv::get();
        #[cfg(feature = "debug")]
        {
            *self.1.status.lock() = FiberStatus::Resumed;
            fiber_log!("resuming fiber {} and suspending {:?}", self, _worker.current_fiber.borrow().0.as_ref().unwrap().id);
            let new = (Some(self.1.clone()), _worker.current_fiber.borrow().0.clone());
            *_worker.current_fiber.borrow_mut() = new;
        }
        let result = {
            #[cfg(windows)]
            unsafe {
                RESUME_INFO.with(|resume_info| {
                    resume_info.give((&mut action, __rayon_core_get_current_fiber()));
                });
                kernel32::SwitchToFiber(self.0 as _);
                mem::forget(self);
                TransferInfo {}
            }
            #[cfg(not(windows))]
            unsafe {
                let t = self.to_context().resume(&mut action as *mut _ as usize);
                TransferInfo {
                    pending_action: t.data,
                    old_context: t.context
                }
            }
        };
        mem::forget(action);
        #[cfg(feature = "tlv")]
        tlv::TLV.with(|tlv| tlv.set(saved_tlv));
        result
    }

    pub fn spawn(worker_thread: &WorkerThread, job: JobRef, action: ResumeAction) -> TransferInfo {
        let stack = {
            let mut cache = worker_thread.stack_cache.borrow_mut();
            if let Some(stack) = cache.pop() {
                stack
            } else {
                let stack_size = worker_thread.registry.stack_size;
                for _i in 0..20 {
                    cache.push(FiberStack::new(stack_size).unwrap());
                }
                FiberStack::new(stack_size).unwrap()
            }
        };

        let mut data = FiberData {
            stack,
            job,
            action,
            #[cfg(windows)]
            context: unsafe { __rayon_core_get_current_fiber() },
        };

        #[cfg(feature = "debug")]
        {
            fiber_log!("using stack {:x} - {:x}", data.stack.bottom() as usize, data.stack.top() as usize);
            let new_id = FIBER_ID.fetch_add(1, Ordering::SeqCst);
            let fiber_data = Arc::new(FiberInfo::new(FiberId::Job(new_id), data.stack.bottom() as usize, data.stack.top() as usize));
            let new = (Some(fiber_data), worker_thread.current_fiber.borrow().0.clone());
            *worker_thread.current_fiber.borrow_mut() = new;
        }

        #[cfg(feature = "tlv")]
        let saved_tlv = tlv::get();

        let result = unsafe {
            #[cfg(windows)]
            {
                NEW_FIBER_DATA.with(|new_fiber_data| {
                    new_fiber_data.give(&mut data as *mut _);
                });
                kernel32::SwitchToFiber(data.stack.0);
                TransferInfo {}
            }

            #[cfg(not(windows))]
            {
                let context = Context::new(&data.stack, context_function);
                let t = context.resume(&mut data as *mut _ as usize);
                TransferInfo {
                    pending_action: t.data,
                    old_context: t.context
                }
            }
        };

        #[cfg(feature = "tlv")]
        tlv::TLV.with(|tlv| tlv.set(saved_tlv));

        mem::forget(data);
        result
    }
}