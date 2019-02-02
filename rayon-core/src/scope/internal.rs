#![cfg(rayon_unstable)]

use super::{Scope, ScopeBase};
use internal::task::{ScopeHandle, Task, ToScopeHandle};
use std::any::Any;
use std::mem;
use std::sync::Arc;

impl<'scope> ToScopeHandle<'scope> for Scope<'scope> {
    type ScopeHandle = LocalScopeHandle<'scope>;

    fn to_scope_handle(&self) -> Self::ScopeHandle {
        unsafe { LocalScopeHandle::new(self) }
    }
}

#[derive(Debug)]
pub struct LocalScopeHandle<'scope> {
    scope: *const ScopeBase<'scope>,
}

impl<'scope> LocalScopeHandle<'scope> {
    /// Caller guarantees that `*scope` will remain valid
    /// until the scope completes. Since we acquire a ref,
    /// that means it will remain valid until we release it.
    unsafe fn new(scope: &Scope<'scope>) -> Self {
        scope.base.increment();
        LocalScopeHandle { scope: &scope.base }
    }
}

impl<'scope> Drop for LocalScopeHandle<'scope> {
    fn drop(&mut self) {
        unsafe {
            if !self.scope.is_null() {
                (*self.scope).job_completed_ok();
            }
        }
    }
}

/// We assert that the `Self` type remains valid until a
/// method is called, and that `'scope` will not end until
/// that point.
unsafe impl<'scope> ScopeHandle<'scope> for LocalScopeHandle<'scope> {
    unsafe fn spawn_task<T: Task + 'scope>(&self, task: Arc<T>) {
        let scope = &*self.scope;
        scope.registry.submit_task(task);
    }

    fn ok(self) {
        mem::drop(self);
    }

    fn panicked(self, err: Box<Any + Send>) {
        unsafe {
            (*self.scope).job_panicked(err);
            mem::forget(self); // no need to run dtor now
        }
    }
}
