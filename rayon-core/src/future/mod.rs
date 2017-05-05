//! Future support in Rayon. This module *primary* consists of
//! internal APIs that are exposed through `Scope::spawn_future` and
//! `::spawn_future`.  However, the type `RayonFuture` is a public
//! type exposed to all users.
//!
//! See `README.md` for details.

use latch::{LatchProbe};
#[allow(warnings)]
use log::Event::*;
use futures::{Async, Poll};
use futures::executor;
use futures::future::CatchUnwind;
use futures::task::{self, Spawn, Task, Unpark};
use job::{Job, JobRef};
use registry::{Registry, WorkerThread};
use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::*;
use std::sync::Mutex;
use unwind;

pub use futures::Future;

const STATE_PARKED: usize = 0;
const STATE_UNPARKED: usize = 1;
const STATE_EXECUTING: usize = 2;
const STATE_EXECUTING_UNPARKED: usize = 3;
const STATE_COMPLETE: usize = 4;

/// Represents the result of a future that has been spawned in the
/// Rayon threadpool.
///
/// # Panic behavior
///
/// Any panics that occur while computing the spawned future will be
/// propagated when this future is polled.
pub struct RayonFuture<T, E> {
    // Warning: Public end-user API!
    inner: Arc<ScopeFutureTrait<Result<T, E>, Box<Any + Send + 'static>>>,
}

/// Unsafe because implementor must guarantee:
///
/// 1. That the type `Self` remains dynamically valid until one of the
///    completion methods is called.
/// 2. That the lifetime `'scope` cannot end until one of those
///    methods is called.
///
/// NB. Although this is public, it is not exposed to outside users.
pub unsafe trait FutureScope<'scope> {
    fn registry(&self) -> Arc<Registry>;
    fn future_panicked(self, err: Box<Any + Send>);
    fn future_completed(self);
}

/// Create a `RayonFuture` that will execute `F` and yield its result,
/// propagating any panics.
///
/// NB. Although this is public, it is not exposed to outside users.
pub fn new_rayon_future<'scope, F, S>(future: F, scope: S) -> RayonFuture<F::Item, F::Error>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{
    let inner = ScopeFuture::spawn(future, scope);

    // We assert that it is safe to hide the type `F` (and, in
    // particular, the lifetimes in it). This is true because the API
    // offered by a `RayonFuture` only permits access to the result of
    // the future (of type `F::Item` or `F::Error`) and those types
    // *are* exposed in the `RayonFuture<F::Item, F::Error>` type. See
    // README.md for details.
    unsafe {
        return RayonFuture { inner: hide_lifetime(inner) };
    }

    unsafe fn hide_lifetime<'l, T, E>(x: Arc<ScopeFutureTrait<T, E> + 'l>)
                                      -> Arc<ScopeFutureTrait<T, E>> {
        mem::transmute(x)
    }
}

impl<T, E> RayonFuture<T, E> {
    pub fn rayon_wait(mut self) -> Result<T, E> {
        // NB: End-user API!
        let worker_thread = WorkerThread::current();
        if worker_thread.is_null() {
            self.wait()
        } else {
            // Assert that uses of `worker_thread` pointer below are
            // valid (because we are on the worker-thread).
            unsafe {
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

    fn wait(self) -> Result<T, E> {
        if WorkerThread::current().is_null() {
            executor::spawn(self).wait_future()
        } else {
            panic!("using  `wait()` in a Rayon thread is unwise; try `rayon_wait()`")
        }
    }

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

struct ScopeFuture<'scope, F, S>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{
    state: AtomicUsize,
    registry: Arc<Registry>,
    contents: Mutex<ScopeFutureContents<'scope, F, S>>,
}

type CU<F> = CatchUnwind<AssertUnwindSafe<F>>;
type CUItem<F> = <CU<F> as Future>::Item;
type CUError<F> = <CU<F> as Future>::Error;

struct ScopeFutureContents<'scope, F, S>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{
    spawn: Option<Spawn<CU<F>>>,
    unpark: Option<Arc<Unpark>>,

    // Pointer to ourselves. We `None` this out when we are finished
    // executing, but it's convenient to keep around normally.
    this: Option<Arc<ScopeFuture<'scope, F, S>>>,

    // the counter in the scope; since the scope doesn't terminate until
    // counter reaches zero, and we hold a ref in this counter, we are
    // assured that this pointer remains valid
    scope: Option<S>,

    waiting_task: Option<Task>,
    result: Poll<CUItem<F>, CUError<F>>,

    canceled: bool,
}

// Assert that the `*const` is safe to transmit between threads:
unsafe impl<'scope, F, S> Send for ScopeFuture<'scope, F, S>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{}
unsafe impl<'scope, F, S> Sync for ScopeFuture<'scope, F, S>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{}

impl<'scope, F, S> ScopeFuture<'scope, F, S>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{
    fn spawn(future: F, scope: S) -> Arc<Self> {
        // Using `AssertUnwindSafe` is valid here because (a) the data
        // is `Send + Sync`, which is our usual boundary and (b)
        // panics will be propagated when the `RayonFuture` is polled.
        let spawn = task::spawn(AssertUnwindSafe(future).catch_unwind());

        let future: Arc<Self> = Arc::new(ScopeFuture::<F, S> {
            state: AtomicUsize::new(STATE_PARKED),
            registry: scope.registry(),
            contents: Mutex::new(ScopeFutureContents {
                spawn: None,
                unpark: None,
                this: None,
                scope: Some(scope),
                waiting_task: None,
                result: Ok(Async::NotReady),
                canceled: false,
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

        future.unpark();

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
        //
        // Here we assert that hiding the lifetimes in this fashion is
        // safe: we claim this is true because the lifetimes we are
        // hiding are part of `F`, and we now that any lifetimes in
        // `F` outlive `counter`. And we can see from `complete()`
        // that we drop all values of type `F` before decrementing
        // `counter`.
        unsafe {
            return hide_lifetime(this.clone());
        }

        unsafe fn hide_lifetime<'l>(x: Arc<Unpark + 'l>) -> Arc<Unpark> {
            mem::transmute(x)
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

                        // Assert that `job_ref` remains valid until
                        // it is executed.  That's true because
                        // `job_ref` holds a ref on the `Arc` and
                        // because, until `job_ref` completes, the
                        // references in the future are valid.
                        unsafe {
                            let job_ref = Self::into_job_ref(contents.this.clone().unwrap());
                            self.registry.inject_or_push(job_ref);
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

impl<'scope, F, S> Unpark for ScopeFuture<'scope, F, S>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{
    fn unpark(&self) {
        self.unpark_inherent();
    }
}

impl<'scope, F, S> Job for ScopeFuture<'scope, F, S>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{
    unsafe fn execute(this: *const Self) {
        let this: Arc<Self> = mem::transmute(this);

        // *generally speaking* there should be no contention for the
        // lock, but it is possible -- we can end execution, get re-enqeueud,
        // and re-executed, before we have time to return from this fn
        let mut contents = this.contents.lock().unwrap();

        log!(FutureExecute { state: this.state.load(Relaxed) });

        this.begin_execute_state();
        loop {
            if contents.canceled {
                return contents.complete(Ok(Async::NotReady));
            } else {
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
}

impl<'scope, F, S> ScopeFutureContents<'scope, F, S>
    where F: Future + Send + 'scope, S: FutureScope<'scope>,
{
    fn poll(&mut self) -> Poll<CUItem<F>, CUError<F>> {
        let unpark = self.unpark.clone().unwrap();
        self.spawn.as_mut().unwrap().poll_future(unpark)
    }

    fn complete(&mut self, value: Poll<CUItem<F>, CUError<F>>) {
        log!(FutureComplete);

        // So, this is subtle. We know that the type `F` may have some
        // data which is only valid until the end of the scope, and we
        // also know that the scope doesn't end until `self.counter`
        // is decremented below. So we want to be sure to drop
        // `self.future` first, lest its dtor try to access some of
        // that state or something!
        mem::drop(self.spawn.take().unwrap());

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

        // `unpark()` here is arbitrary user-code, so it may well
        // panic. We try to capture that panic and forward it
        // somewhere useful if we can.
        let mut err = None;
        if let Some(waiting_task) = self.waiting_task.take() {
            log!(FutureUnparkWaitingTask);
            match unwind::halt_unwinding(|| waiting_task.unpark()) {
                Ok(()) => { }
                Err(e) => { err = Some(e); }
            }
        }

        // Allow the enclosing scope to end. Asserts that
        // `self.counter` is still valid, which we know because caller
        // to `new_rayon_future()` ensures it for us.
        let scope = self.scope.take().unwrap();
        if let Some(err) = err {
            scope.future_panicked(err);
        } else {
            scope.future_completed();
        }
    }
}

impl<'scope, F, S> LatchProbe for ScopeFuture<'scope, F, S>
    where F: Future + Send, S: FutureScope<'scope>,
{
    fn probe(&self) -> bool {
        self.state.load(Acquire) == STATE_COMPLETE
    }
}

/// NB. Although this is public, it is not exposed to outside users.
pub trait ScopeFutureTrait<T, E>: Send + Sync + LatchProbe {
    fn poll(&self) -> Poll<T, E>;
    fn cancel(&self);
}

impl<'scope, F, S> ScopeFutureTrait<CUItem<F>, CUError<F>> for ScopeFuture<'scope, F, S>
    where F: Future + Send, S: FutureScope<'scope>,
{
    fn poll(&self) -> Poll<CUItem<F>, CUError<F>> {
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

        // Slow-path. Get the lock and set the canceled flag to
        // true. Also grab the `unpark` instance (which may be `None`,
        // if the future completes before we get the lack).
        let unpark = {
            let mut contents = self.contents.lock().unwrap();
            contents.canceled = true;
            contents.unpark.clone()
        };

        // If the `unpark` we grabbed was not `None`, then signal it.
        // This will schedule the future.
        if let Some(u) = unpark {
            u.unpark();
        }
    }
}

#[cfg(test)]
mod test;
