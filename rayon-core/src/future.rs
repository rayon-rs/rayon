#![allow(missing_debug_implementations)]
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

pub struct AsyncSpawn<T> {
    state: Arc<Mutex<State<T>>>,
}

struct AsyncSpawnJob<T> {
    state: Arc<Mutex<State<T>>>,
}

struct State<T> {
    result: JobResult<T>,
    waker: Option<Waker>,
}

fn new<T>() -> (AsyncSpawn<T>, AsyncSpawnJob<T>) {
    let state = Arc::new(Mutex::new(State {
        result: JobResult::None,
        waker: None,
    }));
    (
        AsyncSpawn {
            state: state.clone(),
        },
        AsyncSpawnJob { state },
    )
}

impl<T> Future for AsyncSpawn<T> {
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

impl<T> AsyncSpawnJob<T> {
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

pub fn async_spawn<F, T>(func: F) -> AsyncSpawn<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (future, job) = new();
    spawn(move || job.execute(func));
    future
}

pub fn async_spawn_fifo<F, T>(func: F) -> AsyncSpawn<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (future, job) = new();
    spawn_fifo(move || job.execute(func));
    future
}

impl ThreadPool {
    pub fn async_spawn<F, T>(&self, func: F) -> AsyncSpawn<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (future, job) = new();
        self.spawn(move || job.execute(func));
        future
    }

    pub fn async_spawn_fifo<F, T>(&self, func: F) -> AsyncSpawn<T>
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
    pub fn async_spawn<F, T>(&self, func: F) -> AsyncSpawn<T>
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
    pub fn async_spawn_fifo<F, T>(&self, func: F) -> AsyncSpawn<T>
    where
        F: FnOnce(&Self) -> T + Send + 'scope,
        T: Send + 'scope,
    {
        let (future, job) = new();
        self.spawn_fifo(|scope| job.execute(move || func(scope)));
        future
    }
}
