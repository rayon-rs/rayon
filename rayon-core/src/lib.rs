//!
//! [Under construction](https://github.com/rayon-rs/rayon/issues/231)
//!
//! ## Restricting multiple versions
//!
//! In order to ensure proper coordination between threadpools, and especially
//! to make sure there's only one global threadpool, `rayon-core` is actively
//! restricted from building multiple versions of itself into a single target.
//! You may see a build error like this in violation:
//!
//! ```text
//! error: native library `rayon-core` is being linked to by more
//! than one package, and can only be linked to by one package
//! ```
//!
//! While we strive to keep `rayon-core` semver-compatible, it's still
//! possible to arrive at this situation if different crates have overly
//! restrictive tilde or inequality requirements for `rayon-core`.  The
//! conflicting requirements will need to be resolved before the build will
//! succeed.

#![doc(html_root_url = "https://docs.rs/rayon-core/1.5")]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![deny(unreachable_pub)]

use std::any::Any;
use std::env;
use std::error::Error;
use std::fmt;
use std::io;
use std::marker::PhantomData;

extern crate crossbeam_deque;
extern crate crossbeam_queue;
extern crate crossbeam_utils;
#[cfg(any(debug_assertions, rayon_unstable))]
#[macro_use]
extern crate lazy_static;
extern crate num_cpus;

#[cfg(test)]
extern crate rand;
#[cfg(test)]
extern crate rand_xorshift;

#[macro_use]
mod log;

mod job;
mod join;
mod latch;
mod registry;
mod scope;
mod sleep;
mod spawn;
mod thread_pool;
mod unwind;
mod util;

mod compile_fail;
mod test;

#[cfg(rayon_unstable)]
pub mod internal;
pub use join::{join, join_context};
pub use registry::ThreadBuilder;
pub use scope::{scope, Scope};
pub use scope::{scope_fifo, ScopeFifo};
pub use spawn::{spawn, spawn_fifo};
pub use thread_pool::current_thread_has_pending_tasks;
pub use thread_pool::current_thread_index;
pub use thread_pool::ThreadPool;

/// Returns the number of threads in the current registry. If this
/// code is executing within a Rayon thread-pool, then this will be
/// the number of threads for the thread-pool of the current
/// thread. Otherwise, it will be the number of threads for the global
/// thread-pool.
///
/// This can be useful when trying to judge how many times to split
/// parallel work (the parallel iterator traits use this value
/// internally for this purpose).
///
/// # Future compatibility note
///
/// Note that unless this thread-pool was created with a
/// builder that specifies the number of threads, then this
/// number may vary over time in future versions (see [the
/// `num_threads()` method for details][snt]).
///
/// [snt]: struct.ThreadPoolBuilder.html#method.num_threads
pub fn current_num_threads() -> usize {
    ::registry::Registry::current_num_threads()
}

/// Error when initializing a thread pool.
#[derive(Debug)]
pub struct ThreadPoolBuildError {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    GlobalPoolAlreadyInitialized,
    IOError(io::Error),
}

/// Used to create a new [`ThreadPool`] or to configure the global rayon thread pool.
/// ## Creating a ThreadPool
/// The following creates a thread pool with 22 threads.
///
/// ```rust
/// # use rayon_core as rayon;
/// let pool = rayon::ThreadPoolBuilder::new().num_threads(22).build().unwrap();
/// ```
///
/// To instead configure the global thread pool, use [`build_global()`]:
///
/// ```rust
/// # use rayon_core as rayon;
/// rayon::ThreadPoolBuilder::new().num_threads(22).build_global().unwrap();
/// ```
///
/// [`ThreadPool`]: struct.ThreadPool.html
/// [`build_global()`]: struct.ThreadPoolBuilder.html#method.build_global
pub struct ThreadPoolBuilder<B = DefaultBuild> {
    base: ThreadPoolBuilderBase,
    build: B,
}

/// NB: This is INTENTIONALLY a private module -- this hack makes the
/// compiler consider `ThreadPoolBuilderBase` to be public, even
/// though it's not nameable by the end-user.
mod base;
use self::base::ThreadPoolBuilderBase; // re-export

/// Internal trait used by `ThreadPoolBuilder` and
/// `ThreadPoolBuilerExt`. Defines the operation that constructs the
/// thread-pool. As a user of Rayon, you would never need to implement
/// this trait yourself.
pub trait ThreadPoolBuild {
    /// Given a `ThreadPoolBuilder`, construct the actual thread-pool,
    /// spawning all threads.
    fn build_thread_pool(self, builder: ThreadPoolBuilderBase) -> Result<ThreadPool, ThreadPoolBuildError>;
}

/// Contains the rayon thread pool configuration. Use [`ThreadPoolBuilder`] instead.
///
/// [`ThreadPoolBuilder`]: struct.ThreadPoolBuilder.html
#[deprecated(note = "Use `ThreadPoolBuilder`")]
pub struct Configuration {
    builder: ThreadPoolBuilder,
}

/// The type for a panic handling closure. Note that this same closure
/// may be invoked multiple times in parallel.
type PanicHandler = Fn(Box<Any + Send>) + Send + Sync;

/// The type for a closure that gets invoked when a thread starts. The
/// closure is passed the index of the thread on which it is invoked.
/// Note that this same closure may be invoked multiple times in parallel.
type StartHandler = Fn(usize) + Send + Sync;

/// The type for a closure that gets invoked when a thread exits. The
/// closure is passed the index of the thread on which is is invoked.
/// Note that this same closure may be invoked multiple times in parallel.
type ExitHandler = Fn(usize) + Send + Sync;

impl ThreadPoolBuilder<DefaultBuild> {
    /// Creates and returns a valid rayon thread pool builder, but does not initialize it.
    pub fn new() -> ThreadPoolBuilder {
        ThreadPoolBuilder::default()
    }
}

impl<B> ThreadPoolBuilder<B>
where
    B: ThreadPoolBuild,
{
    /// Create a new `ThreadPool` initialized using this configuration.
    pub fn build(self) -> Result<ThreadPool, ThreadPoolBuildError> {
        self.build.build_thread_pool(self.base)
    }

    /// Create a scoped `ThreadPool` initialized using this configuration.
    ///
    /// The threads in this pool will start by calling `wrapper`, which should
    /// do initialization and continue by calling `ThreadBuilder::run()`.
    pub fn build_scoped<W, F, R>(self, wrapper: W, with_pool: F) -> Result<R, ThreadPoolBuildError>
    where
        W: Fn(ThreadBuilder) + Sync, // expected to call `run()`
        F: FnOnce(&ThreadPool) -> R,
    {
        let result = crossbeam_utils::thread::scope(|scope| {
            let wrapper = &wrapper;
            let pool = self.spawn_seq(|thread| {
                let mut builder = scope.builder();
                if let Some(name) = thread.name() {
                    builder = builder.name(name.to_string());
                }
                if let Some(size) = thread.stack_size() {
                    builder = builder.stack_size(size);
                }
                builder.spawn(move |_| wrapper(thread))?;
                Ok(())
            }).build()?;
            Ok(with_pool(&pool))
        });

        match result {
            Ok(result) => result,
            Err(err) => unwind::resume_unwinding(err),
        }
    }

    /// Returns a new builder which will invoke `spawn` to create new
    /// threads.
    ///
    /// Note that the threads will not exit until after the pool is dropped. It
    /// is up to the caller to wait for thread termination if that is important
    /// for any invariants. For instance, threads created in `crossbeam::scope`
    /// will be joined before that scope returns, and this will block indefinitely
    /// if the pool is leaked.
    ///
    /// This is the preferred function to use for custom spawning as
    /// it permits Rayon to spawn the threads in parallel (though we
    /// do not guarantee that we will do so).
    pub fn spawn<F>(
        self,
        spawn: F,
    ) -> ThreadPoolBuilder<CustomBuild<F>>
    where F: Fn(ThreadBuilder) -> io::Result<()> + Send + Sync
    {
        ThreadPoolBuilder {
            base: self.base,
            build: CustomBuild::new(spawn),
        }
    }

    /// Returns a new builder which will invoke `spawn` to create new
    /// threads.
    ///
    /// Note that the threads will not exit until after the pool is dropped. It
    /// is up to the caller to wait for thread termination if that is important
    /// for any invariants. For instance, threads created in `crossbeam::scope`
    /// will be joined before that scope returns, and this will block indefinitely
    /// if the pool is leaked.
    ///
    /// This function guarantees that threads are spawned sequentially
    /// one after the other. This is convenient for some settings.
    pub fn spawn_seq<F>(
        self,
        spawn: F,
    ) -> ThreadPoolBuilder<CustomSeqBuild<F>>
    where F: FnMut(ThreadBuilder) -> io::Result<()>
    {
        ThreadPoolBuilder {
            base: self.base,
            build: CustomSeqBuild::new(spawn),
        }
    }

    /// Initializes the global thread pool. This initialization is
    /// **optional**.  If you do not call this function, the thread pool
    /// will be automatically initialized with the default
    /// configuration. Calling `build_global` is not recommended, except
    /// in two scenarios:
    ///
    /// - You wish to change the default configuration.
    /// - You are running a benchmark, in which case initializing may
    ///   yield slightly more consistent results, since the worker threads
    ///   will already be ready to go even in the first iteration.  But
    ///   this cost is minimal.
    ///
    /// Initialization of the global thread pool happens exactly
    /// once. Once started, the configuration cannot be
    /// changed. Therefore, if you call `build_global` a second time, it
    /// will return an error. An `Ok` result indicates that this
    /// is the first initialization of the thread pool.
    pub fn build_global(self) -> Result<(), ThreadPoolBuildError> {
        let registry = registry::init_global_registry(self.base)?;
        registry.wait_until_primed();
        Ok(())
    }

    /// Initializes the global thread pool using a custom function for spawning
    /// threads.
    ///
    /// Note that the global thread pool doesn't terminate until the entire process
    /// exits! If this is used with something like `crossbeam::scope` that tries to
    /// join threads, that will block indefinitely.
    pub fn spawn_global(
        self,
        spawn: impl FnMut(ThreadBuilder) -> io::Result<()>,
    ) -> Result<(), ThreadPoolBuildError> {
        let registry = registry::spawn_global_registry(self.base, spawn)?;
        registry.wait_until_primed();
        Ok(())
    }

    /// Set a closure which takes a thread index and returns
    /// the thread's name.
    pub fn thread_name<F>(mut self, closure: F) -> Self
    where
        F: FnMut(usize) -> String + 'static,
    {
        self.base.get_thread_name = Some(Box::new(closure));
        self
    }

    /// Set the number of threads to be used in the rayon threadpool.
    ///
    /// If you specify a non-zero number of threads using this
    /// function, then the resulting thread-pools are guaranteed to
    /// start at most this number of threads.
    ///
    /// If `num_threads` is 0, or you do not call this function, then
    /// the Rayon runtime will select the number of threads
    /// automatically. At present, this is based on the
    /// `RAYON_NUM_THREADS` environment variable (if set),
    /// or the number of logical CPUs (otherwise).
    /// In the future, however, the default behavior may
    /// change to dynamically add or remove threads as needed.
    ///
    /// **Future compatibility warning:** Given the default behavior
    /// may change in the future, if you wish to rely on a fixed
    /// number of threads, you should use this function to specify
    /// that number. To reproduce the current default behavior, you
    /// may wish to use the [`num_cpus`
    /// crate](https://crates.io/crates/num_cpus) to query the number
    /// of CPUs dynamically.
    ///
    /// **Old environment variable:** `RAYON_NUM_THREADS` is a one-to-one
    /// replacement of the now deprecated `RAYON_RS_NUM_CPUS` environment
    /// variable. If both variables are specified, `RAYON_NUM_THREADS` will
    /// be prefered.
    pub fn num_threads(mut self, num_threads: usize) -> ThreadPoolBuilder<B> {
        self.base.num_threads = num_threads;
        self
    }

    /// Normally, whenever Rayon catches a panic, it tries to
    /// propagate it to someplace sensible, to try and reflect the
    /// semantics of sequential execution. But in some cases,
    /// particularly with the `spawn()` APIs, there is no
    /// obvious place where we should propagate the panic to.
    /// In that case, this panic handler is invoked.
    ///
    /// If no panic handler is set, the default is to abort the
    /// process, under the principle that panics should not go
    /// unobserved.
    ///
    /// If the panic handler itself panics, this will abort the
    /// process. To prevent this, wrap the body of your panic handler
    /// in a call to `std::panic::catch_unwind()`.
    pub fn panic_handler<H>(mut self, panic_handler: H) -> ThreadPoolBuilder<B>
    where
        H: Fn(Box<Any + Send>) + Send + Sync + 'static,
    {
        self.base.panic_handler = Some(Box::new(panic_handler));
        self
    }

    /// Set the stack size of the worker threads
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.base.stack_size = Some(stack_size);
        self
    }

    /// **(DEPRECATED)** Suggest to worker threads that they execute
    /// spawned jobs in a "breadth-first" fashion.
    ///
    /// Typically, when a worker thread is idle or blocked, it will
    /// attempt to execute the job from the *top* of its local deque of
    /// work (i.e., the job most recently spawned). If this flag is set
    /// to true, however, workers will prefer to execute in a
    /// *breadth-first* fashion -- that is, they will search for jobs at
    /// the *bottom* of their local deque. (At present, workers *always*
    /// steal from the bottom of other worker's deques, regardless of
    /// the setting of this flag.)
    ///
    /// If you think of the tasks as a tree, where a parent task
    /// spawns its children in the tree, then this flag loosely
    /// corresponds to doing a breadth-first traversal of the tree,
    /// whereas the default would be to do a depth-first traversal.
    ///
    /// **Note that this is an "execution hint".** Rayon's task
    /// execution is highly dynamic and the precise order in which
    /// independent tasks are executed is not intended to be
    /// guaranteed.
    ///
    /// This `breadth_first()` method is now deprecated per [RFC #1],
    /// and in the future its effect may be removed. Consider using
    /// [`scope_fifo()`] for a similar effect.
    ///
    /// [RFC #1]: https://github.com/rayon-rs/rfcs/blob/master/accepted/rfc0001-scope-scheduling.md
    /// [`scope_fifo()`]: fn.scope_fifo.html
    #[deprecated(note = "use `scope_fifo` and `spawn_fifo` for similar effect")]
    pub fn breadth_first(mut self) -> Self {
        self.base.breadth_first = true;
        self
    }

    /// Set a callback to be invoked on thread start.
    ///
    /// The closure is passed the index of the thread on which it is invoked.
    /// Note that this same closure may be invoked multiple times in parallel.
    /// If this closure panics, the panic will be passed to the panic handler.
    /// If that handler returns, then startup will continue normally.
    pub fn start_handler<H>(mut self, start_handler: H) -> ThreadPoolBuilder<B>
    where
        H: Fn(usize) + Send + Sync + 'static,
    {
        self.base.start_handler = Some(Box::new(start_handler));
        self
    }

    /// Set a callback to be invoked on thread exit.
    ///
    /// The closure is passed the index of the thread on which it is invoked.
    /// Note that this same closure may be invoked multiple times in parallel.
    /// If this closure panics, the panic will be passed to the panic handler.
    /// If that handler returns, then the thread will exit normally.
    pub fn exit_handler<H>(mut self, exit_handler: H) -> ThreadPoolBuilder<B>
    where
        H: Fn(usize) + Send + Sync + 'static,
    {
        self.base.exit_handler = Some(Box::new(exit_handler));
        self
    }
}

impl Default for ThreadPoolBuilder<DefaultBuild> {
    fn default() -> Self {
        ThreadPoolBuilder { base: Default::default(), build: Default::default() }
    }
}

/// The default builder XXX
#[derive(Debug, Default)]
pub struct DefaultBuild;

impl ThreadPoolBuild for DefaultBuild
{
    fn build_thread_pool(self, builder: ThreadPoolBuilderBase) -> Result<ThreadPool, ThreadPoolBuildError> {
        ThreadPool::build(builder)
    }
}

/// XXX
#[derive(Debug)]
pub struct CustomBuild<F>
    where F: Fn(ThreadBuilder) -> io::Result<()> + Send + Sync
{
    spawn_fn: F,
}

impl<F> CustomBuild<F>
    where F: Fn(ThreadBuilder) -> io::Result<()> + Send + Sync
{
    fn new(spawn_fn: F) -> Self {
        CustomBuild { spawn_fn }
    }
}

impl<F> ThreadPoolBuild for CustomBuild<F>
    where F: Fn(ThreadBuilder) -> io::Result<()> + Send + Sync
{
    fn build_thread_pool(self, builder: ThreadPoolBuilderBase) -> Result<ThreadPool, ThreadPoolBuildError> {
        ThreadPool::build_spawn(builder, self.spawn_fn)
    }
}

/// XXX
#[derive(Debug)]
pub struct CustomSeqBuild<F>
    where F: FnMut(ThreadBuilder) -> io::Result<()>
{
    spawn_fn: F,
}

impl<F> CustomSeqBuild<F>
    where F: FnMut(ThreadBuilder) -> io::Result<()>
{
    fn new(spawn_fn: F) -> Self {
        CustomSeqBuild { spawn_fn }
    }
}

impl<F> ThreadPoolBuild for CustomSeqBuild<F>
    where F: FnMut(ThreadBuilder) -> io::Result<()>
{
    fn build_thread_pool(self, base: ThreadPoolBuilderBase) -> Result<ThreadPool, ThreadPoolBuildError> {
        ThreadPool::build_spawn(base, self.spawn_fn)
    }
}

#[allow(deprecated)]
impl Configuration {
    /// Creates and return a valid rayon thread pool configuration, but does not initialize it.
    pub fn new() -> Configuration {
        Configuration {
            builder: ThreadPoolBuilder::new(),
        }
    }

    /// Deprecated in favor of `ThreadPoolBuilder::build`.
    pub fn build(self) -> Result<ThreadPool, Box<Error + 'static>> {
        self.builder.build().map_err(Box::from)
    }

    /// Deprecated in favor of `ThreadPoolBuilder::thread_name`.
    pub fn thread_name<F>(mut self, closure: F) -> Self
    where
        F: FnMut(usize) -> String + 'static,
    {
        self.builder = self.builder.thread_name(closure);
        self
    }

    /// Deprecated in favor of `ThreadPoolBuilder::num_threads`.
    pub fn num_threads(mut self, num_threads: usize) -> Configuration {
        self.builder = self.builder.num_threads(num_threads);
        self
    }

    /// Deprecated in favor of `ThreadPoolBuilder::panic_handler`.
    pub fn panic_handler<H>(mut self, panic_handler: H) -> Configuration
    where
        H: Fn(Box<Any + Send>) + Send + Sync + 'static,
    {
        self.builder = self.builder.panic_handler(panic_handler);
        self
    }

    /// Deprecated in favor of `ThreadPoolBuilder::stack_size`.
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.builder = self.builder.stack_size(stack_size);
        self
    }

    /// Deprecated in favor of `ThreadPoolBuilder::breadth_first`.
    pub fn breadth_first(mut self) -> Self {
        self.builder = self.builder.breadth_first();
        self
    }

    /// Deprecated in favor of `ThreadPoolBuilder::start_handler`.
    pub fn start_handler<H>(mut self, start_handler: H) -> Configuration
    where
        H: Fn(usize) + Send + Sync + 'static,
    {
        self.builder = self.builder.start_handler(start_handler);
        self
    }

    /// Deprecated in favor of `ThreadPoolBuilder::exit_handler`.
    pub fn exit_handler<H>(mut self, exit_handler: H) -> Configuration
    where
        H: Fn(usize) + Send + Sync + 'static,
    {
        self.builder = self.builder.exit_handler(exit_handler);
        self
    }

    /// Returns a ThreadPoolBuilder with identical parameters.
    fn into_builder(self) -> ThreadPoolBuilder {
        self.builder
    }
}

impl ThreadPoolBuildError {
    fn new(kind: ErrorKind) -> ThreadPoolBuildError {
        ThreadPoolBuildError { kind }
    }
}

impl Error for ThreadPoolBuildError {
    fn description(&self) -> &str {
        match self.kind {
            ErrorKind::GlobalPoolAlreadyInitialized => {
                "The global thread pool has already been initialized."
            }
            ErrorKind::IOError(ref e) => e.description(),
        }
    }
}

impl fmt::Display for ThreadPoolBuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            ErrorKind::IOError(ref e) => e.fmt(f),
            _ => self.description().fmt(f),
        }
    }
}

/// Deprecated in favor of `ThreadPoolBuilder::build_global`.
#[deprecated(note = "use `ThreadPoolBuilder::build_global`")]
#[allow(deprecated)]
pub fn initialize(config: Configuration) -> Result<(), Box<Error>> {
    config.into_builder().build_global().map_err(Box::from)
}

impl<B> fmt::Debug for ThreadPoolBuilder<B>
where
    B: ThreadPoolBuild,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.base, f)
    }
}

impl fmt::Debug for ThreadPoolBuilderBase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ThreadPoolBuilderBase {
            ref num_threads,
            ref get_thread_name,
            ref panic_handler,
            ref stack_size,
            ref start_handler,
            ref exit_handler,
            ref breadth_first,
        } = self;

        // Just print `Some(<closure>)` or `None` to the debug
        // output.
        struct ClosurePlaceholder;
        impl fmt::Debug for ClosurePlaceholder {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("<closure>")
            }
        }
        let get_thread_name = get_thread_name.as_ref().map(|_| ClosurePlaceholder);
        let panic_handler = panic_handler.as_ref().map(|_| ClosurePlaceholder);
        let start_handler = start_handler.as_ref().map(|_| ClosurePlaceholder);
        let exit_handler = exit_handler.as_ref().map(|_| ClosurePlaceholder);

        f.debug_struct("ThreadPoolBuilder")
            .field("num_threads", num_threads)
            .field("get_thread_name", &get_thread_name)
            .field("panic_handler", &panic_handler)
            .field("stack_size", &stack_size)
            .field("start_handler", &start_handler)
            .field("exit_handler", &exit_handler)
            .field("breadth_first", &breadth_first)
            .finish()
    }
}

#[allow(deprecated)]
impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            builder: Default::default(),
        }
    }
}

#[allow(deprecated)]
impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.builder.fmt(f)
    }
}

/// Provides the calling context to a closure called by `join_context`.
#[derive(Debug)]
pub struct FnContext {
    migrated: bool,

    /// disable `Send` and `Sync`, just for a little future-proofing.
    _marker: PhantomData<*mut ()>,
}

impl FnContext {
    #[inline]
    fn new(migrated: bool) -> Self {
        FnContext {
            migrated,
            _marker: PhantomData,
        }
    }
}

impl FnContext {
    /// Returns `true` if the closure was called from a different thread
    /// than it was provided from.
    #[inline]
    pub fn migrated(&self) -> bool {
        self.migrated
    }
}
