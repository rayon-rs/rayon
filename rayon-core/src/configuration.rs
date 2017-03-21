#[allow(unused_imports)]
use log::Event::*;
use std::any::Any;
use std::env;
use std::error::Error;
use std::str::FromStr;
use std::fmt;
use std::sync::Arc;
use registry;
use num_cpus;

/// Contains the rayon thread pool configuration.
pub struct Configuration {
    /// The number of threads in the rayon thread pool.
    /// If zero will use the RAYON_RS_NUM_CPUS environment variable.
    /// If RAYON_RS_NUM_CPUS is invalid or zero will use the default.
    num_threads: usize,

    /// Custom closure, if any, to handle a panic that we cannot propagate
    /// anywhere else.
    panic_handler: Option<PanicHandler>,

    /// Closure to compute the name of a thread.
    get_thread_name: Option<Box<FnMut(usize) -> String>>,

    /// The stack size for the created worker threads
    stack_size: Option<usize>,

    /// Closure invoked on worker thread start.
    start_handler: Option<StartHandler>,

    /// Closure invoked on worker thread exit.
    exit_handler: Option<ExitHandler>,
}

/// The type for a panic handling closure. Note that this same closure
/// may be invoked multiple times in parallel.
pub type PanicHandler = Arc<Fn(Box<Any + Send>) + Send + Sync>;

pub type StartHandler = Arc<Fn() + Send + Sync>;
pub type ExitHandler = Arc<Fn() + Send + Sync>;

impl Configuration {
    /// Creates and return a valid rayon thread pool configuration, but does not initialize it.
    pub fn new() -> Configuration {
        Configuration {
            num_threads: 0,
            get_thread_name: None,
            panic_handler: None,
            stack_size: None,
            start_handler: None,
            exit_handler: None,
        }
    }

    /// Get the number of threads that will be used for the thread
    /// pool. See `set_num_threads` for more information.
    pub fn num_threads(&self) -> usize {
        if self.num_threads > 0 {
            self.num_threads
        } else {
            match env::var("RAYON_RS_NUM_CPUS").ok().and_then(|s| usize::from_str(&s).ok()) {
                Some(x) if x > 0 => x,
                _ => num_cpus::get(),
            }
        }
    }

    /// Get the thread name for the thread with the given index.
    pub fn thread_name(&mut self, index: usize) -> Option<String> {
        self.get_thread_name.as_mut().map(|c| c(index))
    }

    /// Set a closure which takes a thread index and returns
    /// the thread's name.
    pub fn set_thread_name<F>(mut self, closure: F) -> Self
    where F: FnMut(usize) -> String + 'static {
        self.get_thread_name = Some(Box::new(closure));
        self
    }

    /// Set the number of threads to be used in the rayon threadpool.
    /// If `num_threads` is 0 or you do not call this function,
    /// rayon will use the RAYON_RS_NUM_CPUS environment variable.
    /// If RAYON_RS_NUM_CPUS is invalid or is zero, a suitable default will be used.
    /// Currently, the default is one thread per logical CPU.
    pub fn set_num_threads(mut self, num_threads: usize) -> Configuration {
        self.num_threads = num_threads;
        self
    }

    /// Returns (and takes ownership of) the current panic handler.
    /// After this call, no panic handler is registered in the
    /// configuration anymore.
    pub fn panic_handler(&self) -> Option<PanicHandler> {
        self.panic_handler.clone()
    }

    /// Normally, whenever Rayon catches a panic, it tries to
    /// propagate it to someplace sensible, to try and reflect the
    /// semantics of sequential execution. But in some cases,
    /// particularly with the `spawn_async()` APIs, there is no
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
    pub fn set_panic_handler(mut self, panic_handler: PanicHandler) -> Configuration {
        self.panic_handler = Some(panic_handler);
        self
    }

    /// Get the stack size of the worker threads
    pub fn stack_size(&self) -> Option<usize>{
        self.stack_size
    }

    /// Set the stack size of the worker threads
    pub fn set_stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// Returns a copy of the current thread start callback.
    pub fn start_handler(&self) -> Option<StartHandler> {
        self.start_handler.clone()
    }

    pub fn set_start_handler(mut self, start_handler: StartHandler) -> Configuration {
        self.start_handler = Some(start_handler);
        self
    }

    /// Returns a copy of the current thread exit callback.
    pub fn exit_handler(&self) -> Option<ExitHandler> {
        self.exit_handler.clone()
    }

    pub fn set_exit_handler(mut self, exit_handler: ExitHandler) -> Configuration {
        self.exit_handler = Some(exit_handler);
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

/// This is a debugging API not really intended for end users. It will
/// dump some performance statistics out using `println`.
pub fn dump_stats() {
    dump_stats!();
}

impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Configuration { ref num_threads, ref get_thread_name, ref panic_handler, ref stack_size } = *self;
                            ref start_handler, ref exit_handler } = *self;

        // Just print `Some("<closure>")` or `None` to the debug
        // output.
        let get_thread_name = get_thread_name.as_ref().map(|_| "<closure>");

        // Just print `Some("<closure>")` or `None` to the debug
        // output.
        let panic_handler = panic_handler.as_ref().map(|_| "<closure>");
        let start_handler = start_handler.as_ref().map(|_| "<closure>");
        let exit_handler = exit_handler.as_ref().map(|_| "<closure>");

        f.debug_struct("Configuration")
         .field("num_threads", num_threads)
         .field("get_thread_name", &get_thread_name)
         .field("panic_handler", &panic_handler)
         .field("stack_size", &stack_size)
         .field("start_handler", &start_handler)
         .field("exit_handler", &exit_handler)
         .finish()
    }
}
