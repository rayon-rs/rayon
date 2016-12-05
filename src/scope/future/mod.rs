use latch::{CountLatch, Latch, LatchProbe};
#[allow(warnings)]
use log::Event::*;
use futures::{Async, Future, Poll};
use futures::future::CatchUnwind;
use futures::task::{self, Spawn, Task, Unpark};
use job::{Job, JobRef};
use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::*;
use std::sync::Mutex;
use thread_pool::{Registry, WorkerThread};
use unwind;

const STATE_PARKED: usize = 0;
const STATE_UNPARKED: usize = 1;
const STATE_EXECUTING: usize = 2;
const STATE_EXECUTING_UNPARKED: usize = 3;
const STATE_COMPLETE: usize = 4;

// Warning: Public end-user API.
pub struct RayonFuture<T, E> {
    inner: Arc<ScopeFutureTrait<Result<T, E>, Box<Any + Send + 'static>>>,
}

/// This is a free fn so that we can expose `RayonFuture` as public API.
pub unsafe fn new_rayon_future<F>(future: F,
                                  counter: *const CountLatch)
                                  -> RayonFuture<F::Item, F::Error>
    where F: Future + Send
{
    let inner = ScopeFuture::spawn(future, counter);
    RayonFuture { inner: hide_lifetime(inner) }
}

unsafe fn hide_lifetime<'l, T, E>(x: Arc<ScopeFutureTrait<T, E> + 'l>)
                                  -> Arc<ScopeFutureTrait<T, E>> {
    mem::transmute(x)
}

impl<T, E> RayonFuture<T, E> {
    pub fn rayon_wait(mut self) -> Result<T, E> {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                self.wait()
            } else {
                (*worker_thread).wait_until(&*self.inner);
                debug_assert!(self.inner.probe());
                self.poll().map(|a_v| match a_v {
                    Async::Ready(v) => v,
                    Async::NotReady => panic!("probe() returned true but poll not ready")
                })
            }
        }
    }
}

impl<T, E> Future for RayonFuture<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        match self.inner.poll() {
            Ok(Async::Ready(Ok(v))) => Ok(Async::Ready(v)),
            Ok(Async::Ready(Err(e))) => Err(e),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => unwind::resume_unwinding(e),
        }
    }
}

impl<T, E> Drop for RayonFuture<T, E> {
    fn drop(&mut self) {
        self.inner.cancel();
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct ScopeFuture<F: Future + Send> {
    state: AtomicUsize,
    registry: Arc<Registry>,
    contents: Mutex<ScopeFutureContents<F>>,
}

type CU<F> = CatchUnwind<AssertUnwindSafe<F>>;

struct ScopeFutureContents<F: Future + Send> {
    spawn: Option<Spawn<CU<F>>>,
    unpark: Option<Arc<Unpark>>,

    // Pointer to ourselves. We `None` this out when we are finished
    // executing, but it's convenient to keep around normally.
    this: Option<Arc<ScopeFuture<F>>>,

    // the counter in the scope; since the scope doesn't terminate until
    // counter reaches zero, and we hold a ref in this counter, we are
    // assured that this pointer remains valid
    counter: *const CountLatch,

    waiting_task: Option<Task>,
    result: Poll<<CU<F> as Future>::Item, <CU<F> as Future>::Error>,
}

trait Ping: Send + Sync {
    fn ping(&self);
}

// Assert that the `*const` is safe to transmit between threads:
unsafe impl<F: Future + Send> Send for ScopeFuture<F> {}
unsafe impl<F: Future + Send> Sync for ScopeFuture<F> {}

impl<F: Future + Send> ScopeFuture<F> {
    // Unsafe: Caller asserts that `future` and `counter` will remain
    // valid until we invoke `counter.set()`.
    unsafe fn spawn(future: F, counter: *const CountLatch) -> Arc<Self> {
        let worker_thread = WorkerThread::current();
        debug_assert!(!worker_thread.is_null());

        // Using `AssertUnwindSafe` is valid here because (a) the data
        // is `Send + Sync`, which is our usual boundary and (b)
        // panics will be propagated when the `RayonFuture` is polled.
        let spawn = task::spawn(AssertUnwindSafe(future).catch_unwind());

        let future: Arc<Self> = Arc::new(ScopeFuture::<F> {
            state: AtomicUsize::new(STATE_PARKED),
            registry: (*worker_thread).registry().clone(),
            contents: Mutex::new(ScopeFutureContents {
                spawn: None,
                unpark: None,
                this: None,
                counter: counter,
                waiting_task: None,
                result: Ok(Async::NotReady),
            }),
        });

        // Make the two self-cycles. Note that these imply the future
        // cannot be freed until these fields are set to `None` (which
        // occurs when it is finished executing).
        {
            let mut contents = future.contents.try_lock().unwrap();
            contents.spawn = Some(spawn);
            contents.unpark = Some(Self::make_unpark(&future));
            contents.this = Some(future.clone());
        }

        future.ping();

        future
    }

    /// Creates a `JobRef` from this job -- note that this hides all
    /// lifetimes, so it is up to you to ensure that this JobRef
    /// doesn't outlive any data that it closes over.
    unsafe fn into_job_ref(this: Arc<Self>) -> JobRef {
        let this: *const Self = mem::transmute(this);
        JobRef::new(this)
    }

    fn make_unpark(this: &Arc<Self>) -> Arc<Unpark> {
        // Hide any lifetimes in `self`. This is safe because, until
        // `self` is dropped, the counter is not decremented, and so
        // the `'scope` lifetimes cannot end.
        //
        // Unfortunately, as `Unpark` currently requires `'static`, we
        // have to do an indirection and this ultimately requires a
        // fresh allocation.
        unsafe {
            let ping: PingUnpark = PingUnpark::new(this.clone());
            let ping: PingUnpark<'static> = mem::transmute(ping);
            Arc::new(ping)
        }
    }

    fn unpark_inherent(&self) {
        loop {
            match self.state.load(Relaxed) {
                STATE_PARKED => {
                    if {
                        self.state
                            .compare_exchange_weak(STATE_PARKED, STATE_UNPARKED, Release, Relaxed)
                            .is_ok()
                    } {
                        // Contention here is unlikely but possible: a
                        // previous execution might have moved us to the
                        // PARKED state but not yet released the lock.
                        let contents = self.contents.lock().unwrap();
                        unsafe {
                            let job_ref = Self::into_job_ref(contents.this.clone().unwrap());
                            self.registry.inject(&[job_ref]);
                        }
                        return;
                    }
                }

                STATE_EXECUTING => {
                    if {
                        self.state
                            .compare_exchange_weak(STATE_EXECUTING,
                                                   STATE_EXECUTING_UNPARKED,
                                                   Release,
                                                   Relaxed)
                            .is_ok()
                    } {
                        return;
                    }
                }

                state => {
                    debug_assert!(state == STATE_UNPARKED || state == STATE_EXECUTING_UNPARKED ||
                                  state == STATE_COMPLETE);
                    return;
                }
            }
        }
    }

    fn begin_execute_state(&self) {
        // When we are put into the unparked state, we are enqueued in
        // a worker thread. We should then be executed exactly once,
        // at which point we transiition to STATE_EXECUTING. Nobody
        // should be contending with us to change the state here.
        let state = self.state.load(Acquire);
        debug_assert_eq!(state, STATE_UNPARKED);
        let result = self.state.compare_exchange(state, STATE_EXECUTING, Release, Relaxed);
        debug_assert_eq!(result, Ok(STATE_UNPARKED));
    }

    fn end_execute_state(&self) -> bool {
        loop {
            match self.state.load(Relaxed) {
                STATE_EXECUTING => {
                    if {
                        self.state
                            .compare_exchange_weak(STATE_EXECUTING, STATE_PARKED, Release, Relaxed)
                            .is_ok()
                    } {
                        // We put ourselves into parked state, no need to
                        // re-execute.  We'll just wait for the Unpark.
                        return true;
                    }
                }

                state => {
                    debug_assert_eq!(state, STATE_EXECUTING_UNPARKED);
                    if {
                        self.state
                            .compare_exchange_weak(state, STATE_EXECUTING, Release, Relaxed)
                            .is_ok()
                    } {
                        // We finished executing, but an unpark request
                        // came in the meantime.  We need to execute
                        // again. Return false as we failed to end the
                        // execution phase.
                        return false;
                    }
                }
            }
        }
    }
}

impl<F: Future + Send> Ping for ScopeFuture<F> {
    fn ping(&self) {
        self.unpark_inherent();
    }
}

impl<F: Future + Send> Job for ScopeFuture<F> {
    unsafe fn execute(this: *const Self) {
        let this: Arc<Self> = mem::transmute(this);

        // *generally speaking* there should be no contention for the
        // lock, but it is possible -- we can end execution, get re-enqeueud,
        // and re-executed, before we have time to return from this fn
        let mut contents = this.contents.lock().unwrap();

        log!(FutureExecute { state: this.state.load(Relaxed) });

        this.begin_execute_state();
        loop {
            match contents.poll() {
                Ok(Async::Ready(v)) => {
                    log!(FutureExecuteReady);
                    return contents.complete(Ok(Async::Ready(v)));
                }
                Ok(Async::NotReady) => {
                    log!(FutureExecuteNotReady);
                    if this.end_execute_state() {
                        return;
                    }
                }
                Err(err) => {
                    log!(FutureExecuteErr);
                    return contents.complete(Err(err));
                }
            }
        }
    }
}

impl<F: Future + Send> ScopeFutureContents<F> {
    fn poll(&mut self) -> Poll<<CU<F> as Future>::Item, <CU<F> as Future>::Error> {
        let unpark = self.unpark.clone().unwrap();
        self.spawn.as_mut().unwrap().poll_future(unpark)
    }

    fn complete(&mut self, value: Poll<<CU<F> as Future>::Item, <CU<F> as Future>::Error>) {
        log!(FutureComplete);

        // So, this is subtle. We know that the type `F` may have some
        // data which is only valid until the end of the scope, and we
        // also know that the scope doesn't end until `self.counter`
        // is decremented below. So we want to be sure to drop
        // `self.future` first, lest its dtor try to access some of
        // that state or something!
        self.spawn.take().unwrap();

        self.unpark = None;
        self.result = value;
        let this = self.this.take().unwrap();
        if cfg!(debug_assertions) {
            let state = this.state.load(Relaxed);
            debug_assert!(state == STATE_EXECUTING || state == STATE_EXECUTING_UNPARKED,
                          "cannot complete when not executing (state = {})",
                          state);
        }
        this.state.store(STATE_COMPLETE, Release);

        if let Some(waiting_task) = self.waiting_task.take() {
            log!(FutureUnparkWaitingTask);
            waiting_task.unpark();
        }

        // allow the enclosing scope to end
        unsafe {
            (*self.counter).set();
        }
    }
}

struct PingUnpark<'l> {
    ping: Arc<Ping + 'l>,
}

impl<'l> PingUnpark<'l> {
    fn new(ping: Arc<Ping + 'l>) -> PingUnpark {
        PingUnpark { ping: ping }
    }
}

impl Unpark for PingUnpark<'static> {
    fn unpark(&self) {
        self.ping.ping()
    }
}

impl<F> LatchProbe for ScopeFuture<F>
    where F: Future + Send
{
    fn probe(&self) -> bool {
        self.state.load(Acquire) == STATE_COMPLETE
    }
}

pub trait ScopeFutureTrait<T, E>: Send + Sync + LatchProbe {
    fn poll(&self) -> Poll<T, E>;
    fn cancel(&self);
}

impl<F> ScopeFutureTrait<<CU<F> as Future>::Item, <CU<F> as Future>::Error> for ScopeFuture<F>
    where F: Future + Send
{
    fn poll(&self) -> Poll<<CU<F> as Future>::Item, <CU<F> as Future>::Error> {
        // Important: due to transmute hackery, not all the fields are
        // truly known to be valid at this point. In particular, the
        // type F is erased. But the `state` and `result` fields
        // should be valid.
        let mut contents = self.contents.lock().unwrap();
        let state = self.state.load(Relaxed);
        if state == STATE_COMPLETE {
            let r = mem::replace(&mut contents.result, Ok(Async::NotReady));
            return r;
        } else {
            assert!(contents.waiting_task.is_none());
            log!(FutureInstallWaitingTask { state: state });
            contents.waiting_task = Some(task::park());
            Ok(Async::NotReady)
        }
    }

    fn cancel(&self) {
        // Fast-path: check if this is already complete and return if
        // so. A relaxed load suffices since we are not going to
        // access any data as a result of this action.
        if self.state.load(Relaxed) == STATE_COMPLETE {
            return;
        }

        // Slow-path. Get the lock and everything.
        let mut contents = self.contents.lock().unwrap();
        loop {
            match self.state.load(Relaxed) {
                STATE_COMPLETE => {
                    return;
                }

                state => {
                    log!(FutureCancel { state: state });
                    if {
                        self.state
                            .compare_exchange_weak(state, STATE_COMPLETE, Release, Relaxed)
                            .is_ok()
                    } {
                        contents.complete(Ok(Async::NotReady));
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test;
