//! Tracing instrumentation for rayon-core.
//!
//! This module provides optional tracing support, enabled via the `tracing` feature.
//! When enabled, rayon emits events and spans for:
//!
//! - Thread lifecycle (start, exit, idle, active)
//! - Work stealing
//! - Job execution (as spans, enabling duration measurement)
//!
//! All instrumentation compiles to no-ops when the feature is disabled.

/// Emits a tracing event when the `tracing` feature is enabled.
/// Compiles to nothing when disabled.
#[cfg(feature = "tracing")]
macro_rules! trace_event {
    ($($arg:tt)*) => {
        tracing::event!($($arg)*)
    };
}

#[cfg(not(feature = "tracing"))]
macro_rules! trace_event {
    ($($arg:tt)*) => {};
}

/// Creates and enters a tracing span when the `tracing` feature is enabled.
/// Returns an `EnteredSpan` that will exit when dropped.
/// Compiles to nothing when disabled.
#[cfg(feature = "tracing")]
macro_rules! trace_span {
    ($($arg:tt)*) => {
        tracing::span!($($arg)*).entered()
    };
}

#[cfg(not(feature = "tracing"))]
macro_rules! trace_span {
    ($($arg:tt)*) => {
        ()
    };
}

#[cfg(all(test, feature = "tracing"))]
mod tests {
    //! Note: These tests use a global subscriber because rayon's worker threads
    //! don't inherit thread-local subscribers.

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use tracing::span::{Attributes, Id};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

    use crate::ThreadPoolBuilder;

    /// Counters for tracking events and spans.
    struct Counters {
        thread_start: AtomicUsize,
        thread_exit: AtomicUsize,
        thread_idle: AtomicUsize,
        thread_active: AtomicUsize,
        job_stolen: AtomicUsize,
        job_execute_spans: AtomicUsize,
        total_events: AtomicUsize,
    }

    impl Counters {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                thread_start: AtomicUsize::new(0),
                thread_exit: AtomicUsize::new(0),
                thread_idle: AtomicUsize::new(0),
                thread_active: AtomicUsize::new(0),
                job_stolen: AtomicUsize::new(0),
                job_execute_spans: AtomicUsize::new(0),
                total_events: AtomicUsize::new(0),
            })
        }
    }

    /// A simple layer that counts events and spans by name.
    struct CountingLayer(Arc<Counters>);

    impl<S> Layer<S> for CountingLayer
    where
        S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    {
        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            self.0.total_events.fetch_add(1, Ordering::Relaxed);

            // Extract the message field to check event type
            struct MessageVisitor<'a>(&'a Counters);

            impl tracing::field::Visit for MessageVisitor<'_> {
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    if field.name() == "message" {
                        let msg = format!("{:?}", value);
                        if msg.contains("thread_start") {
                            self.0.thread_start.fetch_add(1, Ordering::Relaxed);
                        } else if msg.contains("thread_exit") {
                            self.0.thread_exit.fetch_add(1, Ordering::Relaxed);
                        } else if msg.contains("thread_idle") {
                            self.0.thread_idle.fetch_add(1, Ordering::Relaxed);
                        } else if msg.contains("thread_active") {
                            self.0.thread_active.fetch_add(1, Ordering::Relaxed);
                        } else if msg.contains("job_stolen") {
                            self.0.job_stolen.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }

            event.record(&mut MessageVisitor(&self.0));
        }

        fn on_new_span(&self, attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {
            if attrs.metadata().name().contains("job_execute") {
                self.0.job_execute_spans.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Single test that covers all instrumentation points
    #[test]
    fn test_tracing_instrumentation() {
        // Global counters initialized once
        let counters = Counters::new();
        tracing_subscriber::registry()
            .with(CountingLayer(Arc::clone(&counters)))
            .init();

        // Create a pool and do some work
        let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();

        // Clone the registry so we can wait for threads to stop after pool is dropped
        let registry = Arc::clone(pool.registry());

        pool.install(|| {
            // Do parallel work to trigger events
            crate::join(
                || {
                    // Force some actual work
                    (0..100).sum::<i32>()
                },
                || (100..200).sum::<i32>(),
            );
        });

        drop(pool);

        // Wait for all threads to stop. Since thread_exit events are emitted
        // before the stopped latch is set, this guarantees all events are captured.
        registry.wait_until_stopped();

        // Verify thread lifecycle events
        let starts = counters.thread_start.load(Ordering::Relaxed);
        let exits = counters.thread_exit.load(Ordering::Relaxed);
        assert!(
            starts >= 2,
            "Expected at least 2 thread_start events, got {starts}",
        );
        assert!(
            exits >= 2,
            "Expected at least 2 thread_exit events, got {exits}",
        );

        // Verify job execution spans
        let spans = counters.job_execute_spans.load(Ordering::Relaxed);
        assert!(spans > 0, "Expected some job_execute spans, got {}", spans);
    }
}
