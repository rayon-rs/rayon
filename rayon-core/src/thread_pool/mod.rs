//! Contains support for user-managed thread pools, represented by the
//! the [`ThreadPool`] type (see that struct for details).
//!
//! [`ThreadPool`]: struct.ThreadPool.html

use crate::join;
use crate::registry::{Registry, ThreadSpawn, WorkerThread};
use crate::spawn;
#[allow(deprecated)]
use crate::Configuration;
use crate::{scope, Scope};
use crate::{scope_fifo, ScopeFifo};
use crate::{ThreadPoolBuildError, ThreadPoolBuilder};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

mod test;

/// Represents a user created [thread-pool].
///
/// Use a [`ThreadPoolBuilder`] to specify the number and/or names of threads
/// in the pool. After calling [`ThreadPoolBuilder::build()`], you can then
/// execute functions explicitly within this [`ThreadPool`] using
/// [`ThreadPool::install()`]. By contrast, top level rayon functions
/// (like `join()`) will execute implicitly within the current thread-pool.
///
///
/// ## Creating a ThreadPool
///
/// ```rust
/// # use rayon_core as rayon;
/// let pool = rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap();
/// ```
///
/// [`install()`][`ThreadPool::install()`] executes a closure in one of the `ThreadPool`'s
/// threads. In addition, any other rayon operations called inside of `install()` will also
/// execute in the context of the `ThreadPool`.
///
/// When the `ThreadPool` is dropped, that's a signal for the threads it manages to terminate,
/// they will complete executing any remaining work that you have spawned, and automatically
/// terminate.
///
///
/// [thread-pool]: https://en.wikipedia.org/wiki/Thread_pool
/// [`ThreadPool`]: struct.ThreadPool.html
/// [`ThreadPool::new()`]: struct.ThreadPool.html#method.new
/// [`ThreadPoolBuilder`]: struct.ThreadPoolBuilder.html
/// [`ThreadPoolBuilder::build()`]: struct.ThreadPoolBuilder.html#method.build
/// [`ThreadPool::install()`]: struct.ThreadPool.html#method.install
pub struct ThreadPool {
    registry: Arc<Registry>,
}

impl ThreadPool {
    #[deprecated(note = "Use `ThreadPoolBuilder::build`")]
    #[allow(deprecated)]
    /// Deprecated in favor of `ThreadPoolBuilder::build`.
    pub fn new(configuration: Configuration) -> Result<ThreadPool, Box<dyn Error>> {
        Self::build(configuration.into_builder()).map_err(Box::from)
    }

    pub(super) fn build<S>(
        builder: ThreadPoolBuilder<S>,
    ) -> Result<ThreadPool, ThreadPoolBuildError>
    where
        S: ThreadSpawn,
    {
        let registry = Registry::new(builder)?;
        Ok(ThreadPool { registry })
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
    ///
    /// ## Using `install()`
    ///
    /// ```rust
    ///    # use rayon_core as rayon;
    ///    fn main() {
    ///         let pool = rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap();
    ///         let n = pool.install(|| fib(20));
    ///         println!("{}", n);
    ///    }
    ///
    ///    fn fib(n: usize) -> usize {
    ///         if n == 0 || n == 1 {
    ///             return n;
    ///         }
    ///         let (a, b) = rayon::join(|| fib(n - 1), || fib(n - 2)); // runs inside of `pool`
    ///         return a + b;
    ///     }
    /// ```
    pub fn install<OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce() -> R + Send,
        R: Send,
    {
        self.registry.in_worker(|_, _| op())
    }

    /// Returns the (current) number of threads in the thread pool.
    ///
    /// # Future compatibility note
    ///
    /// Note that unless this thread-pool was created with a
    /// [`ThreadPoolBuilder`] that specifies the number of threads,
    /// then this number may vary over time in future versions (see [the
    /// `num_threads()` method for details][snt]).
    ///
    /// [snt]: struct.ThreadPoolBuilder.html#method.num_threads
    /// [`ThreadPoolBuilder`]: struct.ThreadPoolBuilder.html
    #[inline]
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
    /// # Future compatibility note
    ///
    /// Currently, every thread-pool (including the global
    /// thread-pool) has a fixed number of threads, but this may
    /// change in future Rayon versions (see [the `num_threads()` method
    /// for details][snt]). In that case, the index for a
    /// thread would not change during its lifetime, but thread
    /// indices may wind up being reused if threads are terminated and
    /// restarted.
    ///
    /// [snt]: struct.ThreadPoolBuilder.html#method.num_threads
    #[inline]
    pub fn current_thread_index(&self) -> Option<usize> {
        let curr = self.registry.current_thread()?;
        Some(curr.index())
    }

    /// Returns true if the current worker thread currently has "local
    /// tasks" pending. This can be useful as part of a heuristic for
    /// deciding whether to spawn a new task or execute code on the
    /// current thread, particularly in breadth-first
    /// schedulers. However, keep in mind that this is an inherently
    /// racy check, as other worker threads may be actively "stealing"
    /// tasks from our local deque.
    ///
    /// **Background:** Rayon's uses a [work-stealing] scheduler. The
    /// key idea is that each thread has its own [deque] of
    /// tasks. Whenever a new task is spawned -- whether through
    /// `join()`, `Scope::spawn()`, or some other means -- that new
    /// task is pushed onto the thread's *local* deque. Worker threads
    /// have a preference for executing their own tasks; if however
    /// they run out of tasks, they will go try to "steal" tasks from
    /// other threads. This function therefore has an inherent race
    /// with other active worker threads, which may be removing items
    /// from the local deque.
    ///
    /// [work-stealing]: https://en.wikipedia.org/wiki/Work_stealing
    /// [deque]: https://en.wikipedia.org/wiki/Double-ended_queue
    #[inline]
    pub fn current_thread_has_pending_tasks(&self) -> Option<bool> {
        let curr = self.registry.current_thread()?;
        Some(!curr.local_deque_is_empty())
    }

    /// Execute `oper_a` and `oper_b` in the thread-pool and return
    /// the results. Equivalent to `self.install(|| join(oper_a,
    /// oper_b))`.
    pub fn join<A, B, RA, RB>(&self, oper_a: A, oper_b: B) -> (RA, RB)
    where
        A: FnOnce() -> RA + Send,
        B: FnOnce() -> RB + Send,
        RA: Send,
        RB: Send,
    {
        self.install(|| join(oper_a, oper_b))
    }

    /// Creates a scope that executes within this thread-pool.
    /// Equivalent to `self.install(|| scope(...))`.
    ///
    /// See also: [the `scope()` function][scope].
    ///
    /// [scope]: fn.scope.html
    pub fn scope<'scope, OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce(&Scope<'scope>) -> R + Send,
        R: Send,
    {
        self.install(|| scope(op))
    }

    /// Creates a scope that executes within this thread-pool.
    /// Spawns from the same thread are prioritized in relative FIFO order.
    /// Equivalent to `self.install(|| scope_fifo(...))`.
    ///
    /// See also: [the `scope_fifo()` function][scope_fifo].
    ///
    /// [scope_fifo]: fn.scope_fifo.html
    pub fn scope_fifo<'scope, OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce(&ScopeFifo<'scope>) -> R + Send,
        R: Send,
    {
        self.install(|| scope_fifo(op))
    }

    /// Spawns an asynchronous task in this thread-pool. This task will
    /// run in the implicit, global scope, which means that it may outlast
    /// the current stack frame -- therefore, it cannot capture any references
    /// onto the stack (you will likely need a `move` closure).
    ///
    /// See also: [the `spawn()` function defined on scopes][spawn].
    ///
    /// [spawn]: struct.Scope.html#method.spawn
    pub fn spawn<OP>(&self, op: OP)
    where
        OP: FnOnce() + Send + 'static,
    {
        // We assert that `self.registry` has not terminated.
        unsafe { spawn::spawn_in(op, &self.registry) }
    }

    /// Spawns an asynchronous task in this thread-pool. This task will
    /// run in the implicit, global scope, which means that it may outlast
    /// the current stack frame -- therefore, it cannot capture any references
    /// onto the stack (you will likely need a `move` closure).
    ///
    /// See also: [the `spawn_fifo()` function defined on scopes][spawn_fifo].
    ///
    /// [spawn_fifo]: struct.ScopeFifo.html#method.spawn_fifo
    pub fn spawn_fifo<OP>(&self, op: OP)
    where
        OP: FnOnce() + Send + 'static,
    {
        // We assert that `self.registry` has not terminated.
        unsafe { spawn::spawn_fifo_in(op, &self.registry) }
    }

    /// Returns the current threadpool.
    pub fn current() -> ThreadPool {
        let registry = crate::registry::Registry::current();

        registry.increment_terminate_count();
        ThreadPool { registry }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.registry.terminate();
    }
}

impl fmt::Debug for ThreadPool {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("ThreadPool")
            .field("num_threads", &self.current_num_threads())
            .field("id", &self.registry.id())
            .finish()
    }
}

/// If called from a Rayon worker thread, returns the index of that
/// thread within its current pool; if not called from a Rayon thread,
/// returns `None`.
///
/// The index for a given thread will not change over the thread's
/// lifetime. However, multiple threads may share the same index if
/// they are in distinct thread-pools.
///
/// See also: [the `ThreadPool::current_thread_index()` method].
///
/// [m]: struct.ThreadPool.html#method.current_thread_index
///
/// # Future compatibility note
///
/// Currently, every thread-pool (including the global
/// thread-pool) has a fixed number of threads, but this may
/// change in future Rayon versions (see [the `num_threads()` method
/// for details][snt]). In that case, the index for a
/// thread would not change during its lifetime, but thread
/// indices may wind up being reused if threads are terminated and
/// restarted.
///
/// [snt]: struct.ThreadPoolBuilder.html#method.num_threads
#[inline]
pub fn current_thread_index() -> Option<usize> {
    unsafe {
        let curr = WorkerThread::current().as_ref()?;
        Some(curr.index())
    }
}

/// If called from a Rayon worker thread, indicates whether that
/// thread's local deque still has pending tasks. Otherwise, returns
/// `None`. For more information, see [the
/// `ThreadPool::current_thread_has_pending_tasks()` method][m].
///
/// [m]: struct.ThreadPool.html#method.current_thread_has_pending_tasks
#[inline]
pub fn current_thread_has_pending_tasks() -> Option<bool> {
    unsafe {
        let curr = WorkerThread::current().as_ref()?;
        Some(!curr.local_deque_is_empty())
    }
}
