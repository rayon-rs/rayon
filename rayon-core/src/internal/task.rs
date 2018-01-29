//! Internal, unsafe APIs for creating scoped tasks. Intended for
//! building abstractions atop the rayon-core thread pool, rather than
//! direct use by end users. These APIs are mostly analogous to the
//! (safe) `scope`/`spawn` APIs, but with some unsafe requirements
//! that permit greater efficiency.

use std::any::Any;
use std::sync::Arc;

/// Represents a task that can be scheduled onto the Rayon
/// thread-pool. Once a task is scheduler, it will execute exactly
/// once (eventually).
pub trait Task: Send + Sync {
    /// Invoked by the thread-pool when the task is ready to execute.
    fn execute(this: Arc<Self>);
}

/// Represents a handle onto some Rayon scope. This could be either a
/// local scope created by the `scope()` function or the global scope
/// for a thread-pool. To get a scope-handle, you can invoke
/// `ToScopeHandle::to_scope_handle()` on either a `scope` value or a
/// `ThreadPool`.
///
/// The existence of `ScopeHandler` offers a guarantee:
///
/// - The Rust lifetime `'scope` will not end until the scope-handle
///   is dropped, or until you invoke `panicked()` or `ok()`.
///
/// This trait is intended to be used as follows:
///
/// - You have a parallel task of type `T` to perform where `T: 's`,
///   meaning that any references that `T` contains outlive the lifetime
///   `'s`.
/// - You obtain a scope handle `h` of type `H` where `H:
///   ScopeHandle<'s>`; typically this would be by invoking
///   `to_scope_handle()` on a Rayon scope (of type `Scope<'s>`) or a
///   thread-pool (in which case `'s == 'static`).
/// - You invoke `h.spawn()` to start your job(s). This may be done
///   many times.
///   - Note that `h.spawn()` is an unsafe method. You must ensure
///     that your parallel jobs have completed before moving to
///     the next step.
/// - Eventually, when all invocations are complete, you invoke
///   either `panicked()` or `ok()`.
pub unsafe trait ScopeHandle<'scope>: 'scope {
    /// Enqueues a task for execution within the thread-pool. The task
    /// will eventually be invoked, and once it is, the `Arc` will be
    /// dropped.
    ///
    /// **Unsafe:** The caller must guarantee that the scope handle
    /// (`self`) will not be dropped (nor will `ok()` or `panicked()`
    /// be called) until the task executes. Otherwise, the lifetime
    /// `'scope` may end while the task is still pending.
    unsafe fn spawn_task<T: Task + 'scope>(&self, task: Arc<T>);

    /// Indicates that some sub-task of this scope panicked with the
    /// given `err`. This panic will be propagated back to the user as
    /// appropriate, depending on how this scope handle was derived.
    ///
    /// This takes ownership of the scope handle, meaning that once
    /// you invoke `panicked`, the scope is permitted to terminate
    /// (and, in particular, the Rust lifetime `'scope` may end).
    fn panicked(self, err: Box<Any + Send>);

    /// Indicates that the sub-tasks of this scope that you have
    /// spawned concluded successfully.
    ///
    /// This takes ownership of the scope handle, meaning that once
    /// you invoke `panicked`, the scope is permitted to terminate
    /// (and, in particular, the Rust lifetime `'scope` may end).
    fn ok(self);
}

/// Converts a Rayon structure (typicaly a `Scope` or `ThreadPool`)
/// into a "scope handle". See the `ScopeHandle` trait for more
/// details.
pub trait ToScopeHandle<'scope> {
    /// Scope handle type that gets produced.
    type ScopeHandle: ScopeHandle<'scope>;

    /// Convert the receiver into a scope handle.
    fn to_scope_handle(&self) -> Self::ScopeHandle;
}

