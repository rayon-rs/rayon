#![cfg(rayon_unstable)]

use super::ThreadPool;
use internal::task::{ScopeHandle, Task, ToScopeHandle};
use registry::Registry;
use std::any::Any;
use std::fmt;
use std::sync::Arc;

impl ToScopeHandle<'static> for ThreadPool {
    type ScopeHandle = ThreadPoolScopeHandle;

    fn to_scope_handle(&self) -> Self::ScopeHandle {
        unsafe { ThreadPoolScopeHandle::new(self.registry.clone()) }
    }
}

pub struct ThreadPoolScopeHandle {
    registry: Arc<Registry>,
}

impl fmt::Debug for ThreadPoolScopeHandle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("ThreadPoolScopeHandle")
            .field("pool", &self.registry.id())
            .finish()
    }
}

impl ThreadPoolScopeHandle {
    /// Caller asserts that the registry has not yet terminated.
    unsafe fn new(registry: Arc<Registry>) -> Self {
        registry.increment_terminate_count();
        ThreadPoolScopeHandle { registry }
    }
}

impl Drop for ThreadPoolScopeHandle {
    fn drop(&mut self) {
        self.registry.terminate();
    }
}

/// We assert that:
///
/// (a) the scope valid remains valid until a completion method
///     is called. In this case, "remains valid" means that the
///     registry is not terminated. This is true because we
///     acquire a "termination count" in `StaticFutureScope::new()`
///     which is not released until `future_panicked()` or
///     `future_completed()` is invoked.
/// (b) the lifetime `'static` will not end until a completion
///     method is called. This is true because `'static` doesn't
///     end until the end of the program.
unsafe impl ScopeHandle<'static> for ThreadPoolScopeHandle {
    unsafe fn spawn_task<T: Task + 'static>(&self, task: Arc<T>) {
        self.registry.submit_task(task);
    }

    fn ok(self) {}

    fn panicked(self, err: Box<Any + Send>) {
        self.registry.handle_panic(err);
    }
}
