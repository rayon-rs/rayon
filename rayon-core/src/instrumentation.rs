//! Tracing instrumentation for rayon-core.
//!
//! This module provides optional tracing support, enabled via the `tracing` feature.
//! All instrumentation compiles to no-ops when the feature is disabled.
//!
//! # Spans
//!
//! - `rayon::worker_thread` (DEBUG) - Wraps the entire lifetime of a worker thread.
//!   Fields: `worker` (thread index).
//!
//! - `rayon::job_execute` (DEBUG) - Wraps the execution of a job.
//!   Fields: `job_id`, `worker` (executing thread index).
//!   Parent: The span that was active when the job was created (enables cross-thread
//!   context propagation).
//!
//! # Events
//!
//! - `thread_idle` (TRACE) - Emitted when a worker thread goes idle.
//! - `thread_active` (TRACE) - Emitted when a worker thread wakes up.
//! - `job_injected` (TRACE) - Emitted when a job is injected into the global queue.
//!   Fields: `job_id`.
//! - `job_stolen` (TRACE) - Emitted when a job is stolen from another thread.
//!   Fields: `job_id`, `victim` (thread stolen from).
//!
//! # Context Propagation
//!
//! Jobs capture the current span context at creation time via [`JobContext`]. When
//! a job executes (potentially on a different thread), it re-enters the captured
//! context before creating the `job_execute` span. This allows tracing tools to
//! correctly attribute work to the logical operation that spawned it, even when
//! executed by a different worker thread.

#[cfg(feature = "tracing")]
#[macro_use]
mod inner {
    /// Emits a tracing event when the `tracing` feature is enabled.
    /// Compiles to nothing when disabled.
    macro_rules! trace_event {
        ($($arg:tt)*) => {
            tracing::event!($($arg)*)
        };
    }

    /// Creates and enters a tracing span when the `tracing` feature is enabled.
    /// Returns an `EnteredSpan` that will exit when dropped.
    /// Compiles to nothing when disabled.
    macro_rules! trace_span {
        ($($arg:tt)*) => {
            tracing::span!($($arg)*).entered()
        };
    }

    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(0);

    /// Guard returned by entering a job context.
    pub(crate) type ContextGuard<'a> = tracing::span::Entered<'a>;

    /// Captured context for a job, used to propagate span context across threads.
    #[derive(Clone)]
    pub(crate) struct JobContext {
        span: tracing::Span,
        id: u64,
    }

    impl JobContext {
        /// Captures the current span context and assigns a unique job ID.
        pub(crate) fn current() -> Self {
            Self {
                span: tracing::Span::current(),
                id: NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed),
            }
        }

        /// Returns the unique job ID.
        pub(crate) fn id(&self) -> u64 {
            self.id
        }

        /// Enters the captured span context.
        pub(crate) fn enter(&self) -> ContextGuard<'_> {
            self.span.enter()
        }
    }
}

#[cfg(not(feature = "tracing"))]
#[macro_use]
mod inner {
    macro_rules! trace_event {
        ($($arg:tt)*) => {};
    }

    macro_rules! trace_span {
        ($($arg:tt)*) => {
            ()
        };
    }

    /// Guard returned by entering a job context (no-op).
    pub(crate) struct ContextGuard;

    /// Captured context for a job (no-op when tracing is disabled).
    #[derive(Clone)]
    pub(crate) struct JobContext;

    impl JobContext {
        /// Captures the current span context (no-op).
        pub(crate) fn current() -> Self {
            Self
        }

        /// Returns a placeholder job ID.
        ///
        /// This method exists for API compatibility with the tracing-enabled
        /// version. The value is never used because trace macros expand to
        /// nothing when the feature is disabled.
        #[allow(dead_code)]
        pub(crate) fn id(&self) -> u64 {
            0
        }

        /// No-op context entry.
        pub(crate) fn enter(&self) -> ContextGuard {
            ContextGuard
        }
    }
}

pub(crate) use inner::*;

#[cfg(all(test, feature = "tracing"))]
mod tests {
    //! Note: These tests use a global subscriber because rayon's worker threads
    //! don't inherit thread-local subscribers.

    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};

    use tracing::span::{Attributes, Id};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

    use crate::ThreadPoolBuilder;

    /// Shared test state for tracking spans and events.
    struct TestState {
        // Span/event counters
        worker_thread_spans: AtomicUsize,
        job_execute_spans: AtomicUsize,
        total_events: AtomicUsize,

        // Parent tracking for context propagation tests
        parents: Mutex<HashMap<Id, Option<Id>>>,
        user_span_id: Mutex<Option<Id>>,
        job_spans_with_user_parent: AtomicUsize,
    }

    impl TestState {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                worker_thread_spans: AtomicUsize::new(0),
                job_execute_spans: AtomicUsize::new(0),
                total_events: AtomicUsize::new(0),
                parents: Mutex::new(HashMap::new()),
                user_span_id: Mutex::new(None),
                job_spans_with_user_parent: AtomicUsize::new(0),
            })
        }

        /// Check if `span_id` has `ancestor_id` as an ancestor.
        fn has_ancestor(&self, span_id: &Id, ancestor_id: &Id) -> bool {
            let parents = self.parents.lock().unwrap();
            let mut current = Some(span_id.clone());
            while let Some(id) = current {
                if &id == ancestor_id {
                    return true;
                }
                current = parents.get(&id).and_then(|p| p.clone());
            }
            false
        }
    }

    /// Layer that tracks spans and events for testing.
    struct TestLayer(Arc<TestState>);

    impl<S> Layer<S> for TestLayer
    where
        S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    {
        fn on_event(&self, _event: &Event<'_>, _ctx: Context<'_, S>) {
            self.0.total_events.fetch_add(1, Ordering::Relaxed);
        }

        fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
            let name = attrs.metadata().name();

            // Record parent relationship
            let parent_id = ctx.span(id).and_then(|span| span.parent()).map(|p| p.id());
            self.0.parents.lock().unwrap().insert(id.clone(), parent_id);

            // Count span types
            if name.contains("worker_thread") {
                self.0.worker_thread_spans.fetch_add(1, Ordering::Relaxed);
            } else if name.contains("job_execute") {
                self.0.job_execute_spans.fetch_add(1, Ordering::Relaxed);

                // Check if this job_execute has user_operation as ancestor
                if let Some(ref user_id) = *self.0.user_span_id.lock().unwrap() {
                    if self.0.has_ancestor(id, user_id) {
                        self.0
                            .job_spans_with_user_parent
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            } else if name == "user_operation" {
                *self.0.user_span_id.lock().unwrap() = Some(id.clone());
            }
        }
    }

    /// Returns the shared test state, initializing the global subscriber on first call.
    fn test_state() -> Arc<TestState> {
        static STATE: OnceLock<Arc<TestState>> = OnceLock::new();
        STATE
            .get_or_init(|| {
                let state = TestState::new();
                tracing_subscriber::registry()
                    .with(TestLayer(Arc::clone(&state)))
                    .init();
                state
            })
            .clone()
    }

    /// Test that worker thread and job execution spans are created.
    #[test]
    fn test_tracing_instrumentation() {
        let state = test_state();

        let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
        let registry = Arc::clone(pool.registry());

        pool.install(|| {
            crate::join(|| (0..100).sum::<i32>(), || (100..200).sum::<i32>());
        });

        drop(pool);
        registry.wait_until_stopped();

        // Verify worker thread spans
        let worker_spans = state.worker_thread_spans.load(Ordering::Relaxed);
        assert!(
            worker_spans >= 2,
            "Expected at least 2 worker_thread spans, got {worker_spans}",
        );

        // Verify job execution spans
        let job_spans = state.job_execute_spans.load(Ordering::Relaxed);
        assert!(
            job_spans > 0,
            "Expected some job_execute spans, got {job_spans}"
        );
    }

    /// Test that span context is propagated from job creation site to execution site.
    #[test]
    fn test_context_propagation() {
        let state = test_state();

        let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
        let registry = Arc::clone(pool.registry());

        // Create a user span and spawn work inside it
        let user_span = tracing::span!(tracing::Level::INFO, "user_operation");
        let _enter = user_span.enter();

        pool.install(|| {
            crate::join(|| (0..100).sum::<i32>(), || (100..200).sum::<i32>());
        });

        drop(pool);
        registry.wait_until_stopped();

        // Verify that job_execute spans have user_operation as ancestor
        let jobs_with_parent = state.job_spans_with_user_parent.load(Ordering::Relaxed);
        assert!(
            jobs_with_parent > 0,
            "Expected job_execute spans to have user_operation as ancestor, got {jobs_with_parent}",
        );
    }
}
