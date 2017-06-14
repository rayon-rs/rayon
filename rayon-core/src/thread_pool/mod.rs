use Configuration;
#[cfg(rayon_unstable)]
use future::{Future, RayonFuture};
use latch::LockLatch;
#[allow(unused_imports)]
use log::Event::*;
use job::StackJob;
use join;
use {scope, Scope};
use spawn;
use std::sync::Arc;
use std::error::Error;
use registry::{Registry, WorkerThread};

mod test;
/// # ThreadPool
///
/// The [`ThreadPool`] struct represents a user created [thread-pool]. [`ThreadPool::new()`]
/// takes a [`Configuration`] struct that you can use to specify the number and/or
/// names of threads in the pool. You can then execute functions explicitly within
/// this [`ThreadPool`] using [`ThreadPool::install()`]. By contrast, top level
/// rayon functions (like `join()`)  will execute implicitly within the current thread-pool.
/// 
///
/// ## Creating a ThreadPool
///
/// ```rust
///    # use rayon_core as rayon;
///
///    let pool = rayon::ThreadPool::new(rayon::Configuration::new().num_threads(8)).unwrap();
/// ```
///
/// [`install()`] executes a closure in one of the `ThreadPool`'s threads. In addition, 
/// any other rayon operations called inside of `install()` will also execute in the
/// context of the `ThreadPool`.
///
/// When the `ThreadPool` is dropped, that's a signal for the threads it manages to terminate,
/// they will complete executing any remaining work that you have spawned, and automatically
/// terminate.
///
///
/// [thread-pool]: https://en.wikipedia.org/wiki/Thread_pool
/// [`ThreadPool`]: struct.ThreadPool.html
/// [`ThreadPool::new()`]: struct.ThreadPool.html#method.new
/// [`Configuration`]: struct.Configuration.html
/// [`ThreadPool::install()`]: struct.ThreadPool.html#method.install
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
    #[cfg(rayon_unstable)]
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
    ///
    /// ## Using `install()`
    ///  
    /// ```rust
    ///    # use rayon_core as rayon;
    ///    fn main() {
    ///         let pool = rayon::ThreadPool::new(rayon::Configuration::new().num_threads(8)).unwrap();
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
    #[inline]
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
        unsafe {
            let curr = WorkerThread::current();
            if curr.is_null() {
                None
            } else if (*curr).registry().id() != self.registry.id() {
                None
            } else {
                Some(!(*curr).local_deque_is_empty())
            }
        }
    }

    /// Execute `oper_a` and `oper_b` in the thread-pool and return
    /// the results. Equivalent to `self.install(|| join(oper_a,
    /// oper_b))`.
    pub fn join<A, B, RA, RB>(&self, oper_a: A, oper_b: B) -> (RA, RB)
        where A: FnOnce() -> RA + Send,
              B: FnOnce() -> RB + Send,
              RA: Send,
              RB: Send
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
        where OP: for<'s> FnOnce(&'s Scope<'scope>) -> R + 'scope + Send, R: Send
    {
        self.install(|| scope(op))
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
        where OP: FnOnce() + Send + 'static
    {
        // We assert that `self.registry` has not terminated.
        unsafe { spawn::spawn_in(op, &self.registry) }
    }

    /// Spawns an asynchronous future in the thread pool. `spawn_future()` will inject 
    /// jobs into the threadpool that are not tied to your current stack frame. This means 
    /// `ThreadPool`'s `spawn` methods are not scoped. As a result, it cannot access data
    /// owned by the stack.
    ///
    /// `spawn_future()` returns a `RayonFuture<F::Item, F::Error>`, allowing you to chain
    /// multiple jobs togther.
    ///
    /// ## Using `spawn_future()`
    ///
    /// ```rust
    ///    # extern crate rayon_core as rayon;
    ///    extern crate futures;
    ///    use futures::{future, Future};
    ///    # fn main() {
    ///
    ///    let pool = rayon::ThreadPool::new(rayon::Configuration::new().num_threads(8)).unwrap();
    ///
    ///    let a = pool.spawn_future(future::lazy(move || Ok::<_, ()>(format!("Hello, "))));
    ///    let b = pool.spawn_future(a.map(|mut data| {
    ///                                        data.push_str("world");
    ///                                        data
    ///                                    }));
    ///    let result = b.wait().unwrap(); // `Err` is impossible, so use `unwrap()` here
    ///    println!("{:?}", result); // prints: "Hello, world!"
    ///    # }
    /// ```
    ///
    /// See also: [the `spawn_future()` function defined on scopes][spawn_future].
    ///
    /// [spawn_future]: struct.Scope.html#method.spawn_future
    #[cfg(rayon_unstable)]
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
#[inline]
pub fn current_thread_index() -> Option<usize> {
    unsafe {
        let curr = WorkerThread::current();
        if curr.is_null() {
            None
        } else {
            Some((*curr).index())
        }
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
        let curr = WorkerThread::current();
        if curr.is_null() {
            None
        } else {
            Some(!(*curr).local_deque_is_empty())
        }
    }
}
