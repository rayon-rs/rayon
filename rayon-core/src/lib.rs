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

#![doc(html_root_url = "https://docs.rs/rayon-core/1.3")]
#![allow(non_camel_case_types)] // I prefer to use ALL_CAPS for type parameters
//#![deny(missing_debug_implementations)]
#![cfg_attr(test, feature(conservative_impl_trait))]
#![cfg_attr(feature = "debug", feature(asm))]

#![feature(core_intrinsics)]
#![feature(test)]
#![feature(never_type)]

use std::any::Any;
use std::env;
use std::error::Error;
use std::marker::PhantomData;
use std::str::FromStr;
use std::fmt;

extern crate coco;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate scoped_tls;
extern crate context;
extern crate parking_lot;
extern crate libc;
extern crate num_cpus;
extern crate rand;
#[cfg(feature = "debug")]
extern crate ctrlc;
#[cfg(test)]
extern crate test as bench;
#[cfg(windows)]
extern crate kernel32;
#[cfg(windows)]
extern crate winapi;
extern crate crossbeam_channel;

#[macro_use]
mod log;

pub mod latch;
mod join;
mod job;
pub mod registry;
mod scope;
mod sleep;
mod spawn;
mod test;
mod thread_pool;
mod unwind;
mod util;
pub mod fiber;

#[cfg(rayon_unstable)]
pub mod internal;
pub use thread_pool::ThreadPool;
pub use thread_pool::current_thread_index;
pub use thread_pool::current_thread_has_pending_tasks;
pub use join::{join, join_context};
pub use scope::{scope, Scope};
pub use spawn::spawn;

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
/// ### Future compatibility note
///
/// Note that unless this thread-pool was created with a
/// configuration that specifies the number of threads, then this
/// number may vary over time in future versions (see [the
/// `num_threads()` method for details][snt]).
///
/// [snt]: struct.Configuration.html#method.num_threads
pub fn current_num_threads() -> usize {
    ::registry::Registry::current_num_threads()
}

pub struct PoisonedJob;

/// Contains the rayon thread pool configuration.
#[derive(Default)]
pub struct Configuration {
    /// The number of threads in the rayon thread pool.
    /// If zero will use the RAYON_NUM_THREADS environment variable.
    /// If RAYON_NUM_THREADS is invalid or zero will use the default.
    num_threads: usize,

    /// Custom closure, if any, to handle a panic that we cannot propagate
    /// anywhere else.
    panic_handler: Option<Box<PanicHandler>>,

    /// Closure to compute the name of a thread.
    get_thread_name: Option<Box<FnMut(usize) -> String>>,

    /// The stack size for the created worker threads
    stack_size: Option<usize>,

    /// Closure invoked on deadlock.
    deadlock_handler: Option<Box<DeadlockHandler>>,

    /// Closure invoked on worker thread start.
    start_handler: Option<Box<StartHandler>>,

    /// Closure invoked on worker thread exit.
    exit_handler: Option<Box<ExitHandler>>,

    /// Closure invoked on worker thread start.
    main_handler: Option<Box<MainHandler>>,

    /// If false, worker threads will execute spawned jobs in a
    /// "depth-first" fashion. If true, they will do a "breadth-first"
    /// fashion. Depth-first is the default.
    breadth_first: bool,
}

/// The type for a panic handling closure. Note that this same closure
/// may be invoked multiple times in parallel.
type PanicHandler = Fn(Box<Any + Send>) + Send + Sync;

/// The type for a closure that gets invoked with a
/// function which runs rayon tasks.
/// The closure is passed the index of the thread on which it is invoked.
/// Note that this same closure may be invoked multiple times in parallel.
type MainHandler = Fn(usize, &mut FnMut()) + Send + Sync;

type DeadlockHandler = Fn() + Send + Sync;

/// The type for a closure that gets invoked when a thread starts. The
/// closure is passed the index of the thread on which it is invoked.
/// Note that this same closure may be invoked multiple times in parallel.
type StartHandler = Fn(usize) + Send + Sync;

/// The type for a closure that gets invoked when a thread exits. The
/// closure is passed the index of the thread on which is is invoked.
/// Note that this same closure may be invoked multiple times in parallel.
type ExitHandler = Fn(usize) + Send + Sync;

impl Configuration {
    /// Creates and return a valid rayon thread pool configuration, but does not initialize it.
    pub fn new() -> Configuration {
        Configuration::default()
    }

    /// Create a new `ThreadPool` initialized using this configuration.
    pub fn build(self) -> Result<ThreadPool, Box<Error + 'static>> {
        ThreadPool::new(self)
    }

    /// Get the number of threads that will be used for the thread
    /// pool. See `num_threads()` for more information.
    fn get_num_threads(&self) -> usize {
        if self.num_threads > 0 {
            self.num_threads
        } else {
            match env::var("RAYON_NUM_THREADS").ok().and_then(|s| usize::from_str(&s).ok()) {
                Some(x) if x > 0 => return x,
                Some(x) if x == 0 => return num_cpus::get(),
                _ => {},
            }

            // Support for deprecated `RAYON_RS_NUM_CPUS`.
            match env::var("RAYON_RS_NUM_CPUS").ok().and_then(|s| usize::from_str(&s).ok()) {
                Some(x) if x > 0 => x,
                _ => num_cpus::get(),
            }
        }
    }

    /// Get the thread name for the thread with the given index.
    fn get_thread_name(&mut self, index: usize) -> Option<String> {
        self.get_thread_name.as_mut().map(|c| c(index))
    }

    /// Set a closure which takes a thread index and returns
    /// the thread's name.
    pub fn thread_name<F>(mut self, closure: F) -> Self
    where F: FnMut(usize) -> String + 'static {
        self.get_thread_name = Some(Box::new(closure));
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
    pub fn num_threads(mut self, num_threads: usize) -> Configuration {
        self.num_threads = num_threads;
        self
    }

    /// Returns a copy of the current panic handler.
    fn take_panic_handler(&mut self) -> Option<Box<PanicHandler>> {
        self.panic_handler.take()
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
    pub fn panic_handler<H>(mut self, panic_handler: H) -> Configuration
        where H: Fn(Box<Any + Send>) + Send + Sync + 'static
    {
        self.panic_handler = Some(Box::new(panic_handler));
        self
    }

    /// Get the stack size of the worker threads
    fn get_stack_size(&self) -> Option<usize>{
        self.stack_size
    }

    /// Set the stack size of the worker threads
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// Suggest to worker threads that they execute spawned jobs in a
    /// "breadth-first" fashion. Typically, when a worker thread is
    /// idle or blocked, it will attempt to execute the job from the
    /// *top* of its local deque of work (i.e., the job most recently
    /// spawned). If this flag is set to true, however, workers will
    /// prefer to execute in a *breadth-first* fashion -- that is,
    /// they will search for jobs at the *bottom* of their local
    /// deque. (At present, workers *always* steal from the bottom of
    /// other worker's deques, regardless of the setting of this
    /// flag.)
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
    pub fn breadth_first(mut self) -> Self {
        self.breadth_first = true;
        self
    }

    fn get_breadth_first(&self) -> bool {
        self.breadth_first
    }

    /// Takes the current deadlock callback, leaving `None`.
    fn take_deadlock_handler(&mut self) -> Option<Box<DeadlockHandler>> {
        self.deadlock_handler.take()
    }

    /// Set a callback to be invoked on current deadlock.
    pub fn deadlock_handler<H>(mut self, deadlock_handler: H) -> Configuration
        where H: Fn() + Send + Sync + 'static
    {
        self.deadlock_handler = Some(Box::new(deadlock_handler));
        self
    }

    /// Takes the current thread main callback, leaving `None`.
    fn take_main_handler(&mut self) -> Option<Box<MainHandler>> {
        self.main_handler.take()
    }

    /// Set a callback to be invoked on thread main.
    ///
    /// The closure is passed the index of the thread on which it is invoked.
    /// Note that this same closure may be invoked multiple times in parallel.
    /// If this closure panics, the panic will be passed to the panic handler.
    pub fn main_handler<H>(mut self, main_handler: H) -> Configuration
        where H: Fn(usize, &mut FnMut()) + Send + Sync + 'static
    {
        self.main_handler = Some(Box::new(main_handler));
        self
    }

    /// Takes the current thread start callback, leaving `None`.
    fn take_start_handler(&mut self) -> Option<Box<StartHandler>> {
        self.start_handler.take()
    }

    /// Set a callback to be invoked on thread start.
    ///
    /// The closure is passed the index of the thread on which it is invoked.
    /// Note that this same closure may be invoked multiple times in parallel.
    /// If this closure panics, the panic will be passed to the panic handler.
    /// If that handler returns, then startup will continue normally.
    pub fn start_handler<H>(mut self, start_handler: H) -> Configuration
        where H: Fn(usize) + Send + Sync + 'static
    {
        self.start_handler = Some(Box::new(start_handler));
        self
    }

    /// Returns a current thread exit callback, leaving `None`.
    fn take_exit_handler(&mut self) -> Option<Box<ExitHandler>> {
        self.exit_handler.take()
    }

    /// Set a callback to be invoked on thread exit.
    ///
    /// The closure is passed the index of the thread on which it is invoked.
    /// Note that this same closure may be invoked multiple times in parallel.
    /// If this closure panics, the panic will be passed to the panic handler.
    /// If that handler returns, then the thread will exit normally.
    pub fn exit_handler<H>(mut self, exit_handler: H) -> Configuration
        where H: Fn(usize) + Send + Sync + 'static
    {
        self.exit_handler = Some(Box::new(exit_handler));
        self
    }
}

/// Initializes the global thread pool. This initialization is
/// **optional**.  If you do not call this function, the thread pool
/// will be automatically initialized with the default
/// configuration. In fact, calling `initialize` is not recommended,
/// except for in two scenarios:
///
/// - You wish to change the default configuration.
/// - You are running a benchmark, in which case initializing may
///   yield slightly more consistent results, since the worker threads
///   will already be ready to go even in the first iteration.  But
///   this cost is minimal.
///
/// Initialization of the global thread pool happens exactly
/// once. Once started, the configuration cannot be
/// changed. Therefore, if you call `initialize` a second time, it
/// will return an error. An `Ok` result indicates that this
/// is the first initialization of the thread pool.
pub fn initialize(config: Configuration) -> Result<(), Box<Error>> {
    let registry = try!(registry::init_global_registry(config));
    registry.wait_until_primed();
    Ok(())
}

impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Configuration { ref num_threads, ref get_thread_name,
                            ref panic_handler, ref stack_size,
                            ref start_handler, ref main_handler, ref exit_handler,
                            ref deadlock_handler,
                            ref breadth_first } = *self;

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
        let main_handler = main_handler.as_ref().map(|_| ClosurePlaceholder);
        let exit_handler = exit_handler.as_ref().map(|_| ClosurePlaceholder);
        let deadlock_handler = deadlock_handler.as_ref().map(|_| ClosurePlaceholder);

        f.debug_struct("Configuration")
         .field("num_threads", num_threads)
         .field("get_thread_name", &get_thread_name)
         .field("panic_handler", &panic_handler)
         .field("stack_size", &stack_size)
         .field("start_handler", &start_handler)
         .field("main_handler", &main_handler)
         .field("exit_handler", &exit_handler)
         .field("deadlock_handler", &deadlock_handler)
         .field("breadth_first", &breadth_first)
         .finish()
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
            migrated: migrated,
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
