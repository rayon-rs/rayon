use latch::{CountLatch, Latch};
use futures::{Async, Future, Poll};
use futures::future::CatchUnwind;
use futures::sync::oneshot::{channel, Sender, Receiver};
use futures::task::{self, Spawn, Unpark};
use job::{Job, JobRef};
use std::panic::{self, AssertUnwindSafe};
use std::mem;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::*;
use std::sync::Mutex;
use std::thread;
use thread_pool::{Registry, WorkerThread};

const STATE_PARKED: usize = 0;
const STATE_UNPARKED: usize = 1;
const STATE_EXECUTING: usize = 2;
const STATE_EXECUTING_UNPARKED: usize = 3;
const STATE_COMPLETE: usize = 4;

// Warning: Public end-user API.
pub struct RayonFuture<T, E> {
    receiver: Receiver<thread::Result<Result<T, E>>>,
}

/// This is a free fn so that we can expose `RayonFuture` as public API.
pub unsafe fn new_rayon_future<F>(future: F,
                                  counter: *const CountLatch)
                                  -> RayonFuture<F::Item, F::Error>
    where F: Future + Send
{
    let (tx, rx) = channel();
    ScopeFuture::spawn(future, tx, counter);
    RayonFuture { receiver: rx }
}

impl<T, E> Future for RayonFuture<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        match self.receiver.poll().expect("shouldn't be canceled") {
            Async::Ready(Ok(Ok(e))) => Ok(e.into()),
            Async::Ready(Ok(Err(e))) => Err(e),
            Async::Ready(Err(e)) => panic::resume_unwind(e),
            Async::NotReady => Ok(Async::NotReady),
        }
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
    sender: Option<Sender<thread::Result<Result<F::Item, F::Error>>>>,
    unpark: Option<Arc<Unpark>>,

    // Pointer to ourselves. We `None` this out when we are finished
    // executing, but it's convenient to keep around normally.
    this: Option<Arc<ScopeFuture<F>>>,

    // the counter in the scope; since the scope doesn't terminate until
    // counter reaches zero, and we hold a ref in this counter, we are
    // assured that this pointer remains valid
    counter: *const CountLatch,
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
    unsafe fn spawn(future: F,
                    sender: Sender<thread::Result<Result<F::Item, F::Error>>>,
                    counter: *const CountLatch) {
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
                sender: Some(sender),
                counter: counter,
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
            let state = self.state.load(Acquire);
            if {
                state == STATE_PARKED &&
                self.state
                    .compare_exchange_weak(state, STATE_UNPARKED, Release, Relaxed)
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
            } else if {
                state == STATE_EXECUTING &&
                self.state
                    .compare_exchange_weak(state, STATE_EXECUTING_UNPARKED, Release, Relaxed)
                    .is_ok()
            } {
                return;
            } else {
                debug_assert!(state == STATE_UNPARKED || state == STATE_EXECUTING_UNPARKED ||
                              state == STATE_COMPLETE);
                return;
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
            let state = self.state.load(Acquire);
            if state == STATE_EXECUTING {
                if {
                    self.state
                        .compare_exchange_weak(state, STATE_PARKED, Release, Relaxed)
                        .is_ok()
                } {
                    // We put ourselves into parked state, no need to
                    // re-execute.  We'll just wait for the Unpark.
                    return true;
                }
            } else {
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

        this.begin_execute_state();
        loop {
            match contents.poll() {
                Ok(Async::Ready(v)) => {
                    return contents.complete(Ok(v));
                }
                Ok(Async::NotReady) => {
                    if this.end_execute_state() {
                        return;
                    }
                }
                Err(err) => {
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

    fn complete(&mut self, value: thread::Result<Result<F::Item, F::Error>>) {
        self.unpark = None;
        self.sender.take().unwrap().complete(value);
        let this = self.this.take().unwrap();
        if cfg!(debug_assertions) {
            let state = this.state.load(Relaxed);
            debug_assert!(state == STATE_EXECUTING || state == STATE_EXECUTING_UNPARKED,
                          "cannot complete when not executing (state = {})",
                          state);
        }
        this.state.store(STATE_COMPLETE, Release);
    }
}

impl<F: Future + Send> Drop for ScopeFuture<F> {
    fn drop(&mut self) {
        unsafe {
            // can't be any contention for this lock in drop
            let mut contents = self.contents.try_lock().unwrap();

            // So, this is subtle. We know that the type `F` may have
            // some data which is only valid until the end of the
            // scope, and we also know that the scope doesn't end
            // until `self.counter` is decremented below. So we want
            // to be sure to drop `self.future` first.
            mem::drop(contents.spawn.take());

            // Set the latch. By the struct invariant, we know that
            // the counter pointer will remain valid until this ref is
            // dropped.
            (*contents.counter).set();
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
