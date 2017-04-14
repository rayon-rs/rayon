use Configuration;
#[cfg(feature = "unstable")]
use future::{Future, RayonFuture};
use latch::LockLatch;
#[allow(unused_imports)]
use log::Event::*;
use job::StackJob;
use join;
use {scope, Scope};
#[cfg(feature = "unstable")]
use spawn;
use std::sync::Arc;
use std::error::Error;
use registry::{Registry, WorkerThread};

mod test;

pub struct ThreadPool {
    registry: Arc<Registry>,
}

impl ThreadPool {
    /// Constructs a new thread pool with the given configuration. If
    /// the configuration is not valid, returns a suitable `Err`
    /// result.  See `InitError` for more details.
    pub fn new(configuration: Configuration) -> Result<ThreadPool, Box<Error>> {
        let registry = try!(Registry::new(configuration));
        Ok(ThreadPool { registry: registry })
    }

    /// Returns a handle to the global thread pool. This is the pool
    /// that Rayon will use by default when you perform a `join()` or
    /// `scope()` operation, if no other thread-pool is installed. If
    /// no global thread-pool has yet been started when this function
    /// is called, then the global thread-pool will be created (with
    /// the default configuration). If you wish to configure the
    /// global thread-pool differently, then you can use [the
    /// `rayon::initialize()` function][f] to do so.
    ///
    /// [f]: fn.initialize.html
    #[cfg(feature = "unstable")]
    pub fn global() -> &'static Arc<ThreadPool> {
        lazy_static! {
            static ref DEFAULT_THREAD_POOL: Arc<ThreadPool> =
                Arc::new(ThreadPool { registry: Registry::global() });
        }

        &DEFAULT_THREAD_POOL
    }

    /// Executes `op` within the threadpool. Any attempts to use
    /// `join`, `scope`, or parallel iterators will then operate
    /// within that threadpool.
    ///
    /// # Warning: thread-local data
    ///
    /// Because `op` is executing within the Rayon thread-pool,
    /// thread-local data from the current thread will not be
    /// accessible.
    ///
    /// # Panics
    ///
    /// If `op` should panic, that panic will be propagated.
    pub fn install<OP, R>(&self, op: OP) -> R
        where OP: FnOnce() -> R + Send
    {
        unsafe {
            let job_a = StackJob::new(op, LockLatch::new());
            self.registry.inject(&[job_a.as_job_ref()]);
            job_a.latch.wait();
            job_a.into_result()
        }
    }

    /// Returns the (current) number of threads in the thread pool.
    ///
    /// ### Future compatibility note
    ///
    /// Note that unless this thread-pool was created with a
    /// configuration that specifies the number of threads, then this
    /// number may vary over time in future versions (see [the
    /// `num_threads()` method for details][snt]).
    ///
    /// [snt]: struct.Configuration.html#method.num_threads
    pub fn current_num_threads(&self) -> usize {
        self.registry.num_threads()
    }

    /// If called from a Rayon worker thread in this thread-pool,
    /// returns the index of that thread; if not called from a Rayon
    /// thread, or called from a Rayon thread that belongs to a
    /// different thread-pool, returns `None`.
    ///
    /// The index for a given thread will not change over the thread's
    /// lifetime. However, multiple threads may share the same index if
    /// they are in distinct thread-pools.
    ///
    /// ### Future compatibility note
    ///
    /// Currently, every thread-pool (including the global
    /// thread-pool) has a fixed number of threads, but this may
    /// change in future Rayon versions (see [the `num_threads()` method
    /// for details][snt]). In that case, the index for a
    /// thread would not change during its lifetime, but thread
    /// indices may wind up being reused if threads are terminated and
    /// restarted.
    ///
    /// [snt]: struct.Configuration.html#method.num_threads
    pub fn current_thread_index(&self) -> Option<usize> {
        unsafe {
            let curr = WorkerThread::current();
            if curr.is_null() {
                None
            } else if (*curr).registry().id() != self.registry.id() {
                None
            } else {
                Some((*curr).index())
            }
        }
    }

    /// Execute `oper_a` and `oper_b` in the thread-pool and return
    /// the results. Equivalent to `self.install(|| join(oper_a,
    /// oper_b))`.
    #[cfg(feature = "unstable")]
    pub fn join<A, B, RA, RB>(&self, oper_a: A, oper_b: B) -> (RA, RB)
        where A: FnOnce() -> RA + Send,
              B: FnOnce() -> RB + Send,
              RA: Send,
              RB: Send
    {
        self.install(|| join(oper_a, oper_b))
    }

    /// Execute `oper_a` and `oper_b` in the thread-pool and return
    /// the results. Equivalent to `self.install(|| scope(...))`.
    #[cfg(feature = "unstable")]
    pub fn scope<'scope, OP, R>(&self, op: OP) -> R
        where OP: for<'s> FnOnce(&'s Scope<'scope>) -> R + 'scope + Send, R: Send
    {
        self.install(|| scope(op))
    }

    /// Spawns an asynchronous task in this thread-pool. See `spawn()`
    /// for more details.
    #[cfg(feature = "unstable")]
    pub fn spawn<OP>(&self, op: OP)
        where OP: FnOnce() + Send + 'static
    {
        // We assert that `self.registry` has not terminated.
        unsafe { spawn::spawn_in(op, &self.registry) }
    }

    /// Spawns an asynchronous future in this thread-pool. See
    /// `spawn_future()` for more details.
    #[cfg(feature = "unstable")]
    pub fn spawn_future<F>(&self, future: F) -> RayonFuture<F::Item, F::Error>
        where F: Future + Send + 'static
    {
        // We assert that `self.registry` has not yet terminated.
        unsafe { spawn::spawn_future_in(future, self.registry.clone()) }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.registry.terminate();
    }
}

