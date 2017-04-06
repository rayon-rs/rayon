use Configuration;
#[cfg(feature = "unstable")]
use future::{Future, RayonFuture};
use latch::LockLatch;
#[allow(unused_imports)]
use log::Event::*;
use job::StackJob;
#[cfg(feature = "unstable")]
use spawn_async;
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

    /// Spawns an asynchronous task in this thread-pool. See
    /// `spawn_async()` for more details.
    #[cfg(feature = "unstable")]
    pub fn spawn_async<OP>(&self, op: OP)
        where OP: FnOnce() + Send + 'static
    {
        // We assert that `self.registry` has not terminated.
        unsafe { spawn_async::spawn_async_in(op, &self.registry) }
    }

    /// Spawns an asynchronous task in this thread-pool. See
    /// `spawn_future_async()` for more details.
    #[cfg(feature = "unstable")]
    pub fn spawn_future_async<F>(&self, future: F) -> RayonFuture<F::Item, F::Error>
        where F: Future + Send + 'static
    {
        // We assert that `self.registry` has not yet terminated.
        unsafe { spawn_async::spawn_future_async_in(future, self.registry.clone()) }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.registry.terminate();
    }
}
