#![allow(missing_docs)]

use crate::job::JobResult;
use crate::unwind;
use crate::ThreadPool;
use crate::{spawn, spawn_fifo};
use crate::{Scope, ScopeFifo};

use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

struct RayonFuture<T> {
    state: Arc<Mutex<State<T>>>,
}

struct RayonFutureJob<T> {
    state: Arc<Mutex<State<T>>>,
}

struct State<T> {
    result: JobResult<T>,
    waker: Option<Waker>,
}

fn new<T>() -> (RayonFuture<T>, RayonFutureJob<T>) {
    let state = Arc::new(Mutex::new(State {
        result: JobResult::None,
        waker: None,
    }));
    (
        RayonFuture {
            state: state.clone(),
        },
        RayonFutureJob { state },
    )
}

impl<T> Future for RayonFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut guard = self.state.lock().expect("rayon future lock");
        match mem::replace(&mut guard.result, JobResult::None) {
            JobResult::None => {
                guard.waker = Some(cx.waker().clone());
                Poll::Pending
            }
            JobResult::Ok(x) => Poll::Ready(x),
            JobResult::Panic(p) => {
                drop(guard); // don't poison the lock
                unwind::resume_unwinding(p);
            }
        }
    }
}

impl<T> RayonFutureJob<T> {
    fn execute(self, func: impl FnOnce() -> T) {
        let result = unwind::halt_unwinding(func);
        let mut guard = self.state.lock().expect("rayon future lock");
        guard.result = match result {
            Ok(x) => JobResult::Ok(x),
            Err(p) => JobResult::Panic(p),
        };
        if let Some(waker) = guard.waker.take() {
            waker.wake();
        }
    }
}

pub fn spawn_future<F, T>(func: F) -> impl Future<Output = T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (future, job) = new();
    spawn(move || job.execute(func));
    future
}

pub fn spawn_fifo_future<F, T>(func: F) -> impl Future<Output = T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (future, job) = new();
    spawn_fifo(move || job.execute(func));
    future
}

impl ThreadPool {
    pub fn spawn_future<F, T>(&self, func: F) -> impl Future<Output = T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (future, job) = new();
        self.spawn(move || job.execute(func));
        future
    }

    pub fn spawn_fifo_future<F, T>(&self, func: F) -> impl Future<Output = T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (future, job) = new();
        self.spawn_fifo(move || job.execute(func));
        future
    }
}

impl<'scope> Scope<'scope> {
    pub fn spawn_future<F, T>(&self, func: F) -> impl Future<Output = T>
    where
        F: FnOnce(&Self) -> T + Send + 'scope,
        T: Send + 'scope,
    {
        let (future, job) = new();
        self.spawn(|scope| job.execute(move || func(scope)));
        future
    }
}

impl<'scope> ScopeFifo<'scope> {
    pub fn spawn_fifo_future<F, T>(&self, func: F) -> impl Future<Output = T>
    where
        F: FnOnce(&Self) -> T + Send + 'scope,
        T: Send + 'scope,
    {
        let (future, job) = new();
        self.spawn_fifo(|scope| job.execute(move || func(scope)));
        future
    }
}
