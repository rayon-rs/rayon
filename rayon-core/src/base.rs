use std::str::FromStr;
use super::*;

/// This "public" struct contains the details from a thread-pool builder.
#[derive(Default)]
pub struct ThreadPoolBuilderBase {
    /// The number of threads in the rayon thread pool.
    /// If zero will use the RAYON_NUM_THREADS environment variable.
    /// If RAYON_NUM_THREADS is invalid or zero will use the default.
    pub(super) num_threads: usize,

    /// Custom closure, if any, to handle a panic that we cannot propagate
    /// anywhere else.
    pub(super) panic_handler: Option<Box<PanicHandler>>,

    /// Closure to compute the name of a thread.
    pub(super) get_thread_name: Option<Box<FnMut(usize) -> String>>,

    /// The stack size for the created worker threads
    pub(super) stack_size: Option<usize>,

    /// Closure invoked on worker thread start.
    pub(super) start_handler: Option<Box<StartHandler>>,

    /// Closure invoked on worker thread exit.
    pub(super) exit_handler: Option<Box<ExitHandler>>,

    /// If false, worker threads will execute spawned jobs in a
    /// "depth-first" fashion. If true, they will do a "breadth-first"
    /// fashion. Depth-first is the default.
    pub(super) breadth_first: bool,
}

impl ThreadPoolBuilderBase {
    /// Get the number of threads that will be used for the thread
    /// pool. See `num_threads()` for more information.
    pub(super) fn get_num_threads(&self) -> usize {
        if self.num_threads > 0 {
            self.num_threads
        } else {
            match env::var("RAYON_NUM_THREADS")
                .ok()
                .and_then(|s| usize::from_str(&s).ok())
            {
                Some(x) if x > 0 => return x,
                Some(x) if x == 0 => return num_cpus::get(),
                _ => {}
            }

            // Support for deprecated `RAYON_RS_NUM_CPUS`.
            match env::var("RAYON_RS_NUM_CPUS")
                .ok()
                .and_then(|s| usize::from_str(&s).ok())
            {
                Some(x) if x > 0 => x,
                _ => num_cpus::get(),
            }
        }
    }

    /// Get the thread name for the thread with the given index.
    pub(super) fn get_thread_name(&mut self, index: usize) -> Option<String> {
        let f = self.get_thread_name.as_mut()?;
        Some(f(index))
    }


    /// Get the stack size of the worker threads
    pub(super) fn get_stack_size(&self) -> Option<usize> {
        self.stack_size
    }

    pub(super) fn get_breadth_first(&self) -> bool {
        self.breadth_first
    }

    /// Takes the current thread start callback, leaving `None`.
    pub(super) fn take_start_handler(&mut self) -> Option<Box<StartHandler>> {
        self.start_handler.take()
    }

    /// Returns a current thread exit callback, leaving `None`.
    pub(super) fn take_exit_handler(&mut self) -> Option<Box<ExitHandler>> {
        self.exit_handler.take()
    }

    /// Returns a copy of the current panic handler.
    pub(super) fn take_panic_handler(&mut self) -> Option<Box<PanicHandler>> {
        self.panic_handler.take()
    }
}

