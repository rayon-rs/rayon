use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct AtomicCounters {
    /// Packs together a number of counters.
    ///
    /// Consider the value `0x11112222_3333_4444`:
    ///
    /// - The bits 0x11112222 are the **jobs event counter**.
    /// - The bits 0x3333 are the number of **inactive threads**.
    /// - The bits 0x4444 are the number of **sleeping threads**.
    ///
    /// See the struct `Counters` below.
    value: AtomicU64,
}

#[derive(Copy, Clone)]
pub(super) struct Counters {
    word: u64,
}

/// A value read from the **Jobs Event Counter**.
/// See the [`README.md`](README.md) for more
/// coverage of how the jobs event counter works.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub(super) struct JobsEventCounter(usize);

impl JobsEventCounter {
    pub(super) const MAX: JobsEventCounter = JobsEventCounter(JEC_MAX);

    pub(super) fn as_usize(self) -> usize {
        self.0
    }
}

/// Constant that can be added to add one sleeping thread.
const ONE_SLEEPING: u64 = 0x0000_0000_0000_0001;

/// Constant that can be added to add one inactive thread.
/// An inactive thread is either idle, sleepy, or sleeping.
const ONE_INACTIVE: u64 = 0x0000_0000_0001_0000;

/// Constant that can be added to add one to the JEC.
const ONE_JEC: u64 = 0x0000_0001_0000_0000;

/// Mask to zero out the JEC (but retain everything else).
const ZERO_JEC_MASK: u64 = 0x0000_0000_FFFF_FFFF;

/// Bits to shift to select the sleeping threads
/// (used with `select_bits`).
const SLEEPING_SHIFT: u64 = 0;

/// Bits to shift to select the inactive threads
/// (used with `select_bits`).
const INACTIVE_SHIFT: u64 = 1 * 16;

/// Bits to shift to select the JEC
/// (use JOBS_BITS).
const JEC_SHIFT: u64 = 2 * 16;

/// Max value for the jobs event counter.
const JEC_MAX: usize = 0xFFFF;

/// Max value for the thread counters.
const THREADS_MAX: usize = 0xFFFF;

/// Mask to select the value of a thread counter
/// (sleepy, inactive), after shifting (used with
/// `select_bits`)
const THREADS_BITS: usize = 0xFFFF;

/// Mask to select the JEC, after
/// shifting (used with `select_bits`)
const JEC_BITS: usize = std::usize::MAX;

impl AtomicCounters {
    pub(super) fn new() -> AtomicCounters {
        AtomicCounters {
            value: AtomicU64::new(0),
        }
    }

    /// Load and return the current value of the various counters.
    /// This value can then be given to other method which will
    /// attempt to update the counters via compare-and-swap.
    pub(super) fn load(&self, ordering: Ordering) -> Counters {
        Counters::new(self.value.load(ordering))
    }

    #[inline]
    fn try_exchange(&self, old_value: Counters, new_value: Counters, ordering: Ordering) -> bool {
        self.value
            .compare_exchange(old_value.word, new_value.word, ordering, Ordering::Relaxed)
            .is_ok()
    }

    /// Adds an inactive thread. This cannot fail.
    ///
    /// This should be invoked when a thread enters its idle loop looking
    /// for work. It is decremented when work is found. Note that it is
    /// not decremented if the thread transitions from idle to sleepy or sleeping;
    /// so the number of inactive threads is always greater-than-or-equal
    /// to the number of sleeping threads.
    #[inline]
    pub(super) fn add_inactive_thread(&self) {
        self.value.fetch_add(ONE_INACTIVE, Ordering::SeqCst);
    }

    /// Attempts to increment the jobs event counter by one, returning true
    /// if it succeeded. This can be used for two purposes:
    ///
    /// * If a thread is getting sleepy, and the JEC is even, then it will attempt
    ///   to increment to an odd value.
    /// * If a thread is publishing work, and the JEC is odd, then it will attempt
    ///   to increment to an event value.
    pub(super) fn try_increment_jobs_event_counter(&self, old_value: Counters) -> bool {
        // FIXME -- we should remove the `MAX` constant and just let rollover happen naturally
        let new_value = if old_value.jobs_counter() == JobsEventCounter::MAX {
            Counters::new(old_value.word & ZERO_JEC_MASK)
        } else {
            Counters::new(old_value.word + ONE_JEC)
        };
        self.try_exchange(old_value, new_value, Ordering::SeqCst)
    }

    /// Subtracts an inactive thread. This cannot fail. It is invoked
    /// when a thread finds work and hence becomes active. It returns the
    /// number of sleeping threads to wake up (if any).
    ///
    /// See `add_inactive_thread`.
    pub(super) fn sub_inactive_thread(&self) -> usize {
        let old_value = Counters::new(self.value.fetch_sub(ONE_INACTIVE, Ordering::SeqCst));
        debug_assert!(
            old_value.inactive_threads() > 0,
            "sub_inactive_thread: old_value {:?} has no inactive threads",
            old_value,
        );
        debug_assert!(
            old_value.sleeping_threads() <= old_value.inactive_threads(),
            "sub_inactive_thread: old_value {:?} had {} sleeping threads and {} inactive threads",
            old_value,
            old_value.sleeping_threads(),
            old_value.inactive_threads(),
        );

        // Current heuristic: whenever an inactive thread goes away, if
        // there are any sleeping threads, wake 'em up.
        let sleeping_threads = old_value.sleeping_threads();
        std::cmp::min(sleeping_threads, 2)
    }

    /// Subtracts a sleeping thread. This cannot fail, but it is only
    /// safe to do if you you know the number of sleeping threads is
    /// non-zero (i.e., because you have just awoken a sleeping
    /// thread).
    #[inline]
    pub(super) fn sub_sleeping_thread(&self) {
        let old_value = Counters::new(self.value.fetch_sub(ONE_SLEEPING, Ordering::SeqCst));
        debug_assert!(
            old_value.sleeping_threads() > 0,
            "sub_sleeping_thread: old_value {:?} had no sleeping threads",
            old_value,
        );
        debug_assert!(
            old_value.sleeping_threads() <= old_value.inactive_threads(),
            "sub_sleeping_thread: old_value {:?} had {} sleeping threads and {} inactive threads",
            old_value,
            old_value.sleeping_threads(),
            old_value.inactive_threads(),
        );
    }

    #[inline]
    pub(super) fn try_add_sleeping_thread(&self, old_value: Counters) -> bool {
        debug_assert!(
            old_value.inactive_threads() > 0,
            "try_add_sleeping_thread: old_value {:?} has no inactive threads",
            old_value,
        );
        debug_assert!(
            old_value.sleeping_threads() < THREADS_MAX,
            "try_add_sleeping_thread: old_value {:?} has too many sleeping threads",
            old_value,
        );

        let mut new_value = old_value;
        new_value.word += ONE_SLEEPING;

        self.try_exchange(old_value, new_value, Ordering::SeqCst)
    }
}

fn select_bits(word: u64, shift: u64, bits: usize) -> usize {
    ((word >> shift) as usize) & bits
}

impl Counters {
    fn new(word: u64) -> Counters {
        Counters { word }
    }

    pub(super) fn jobs_counter(self) -> JobsEventCounter {
        JobsEventCounter(select_bits(self.word, JEC_SHIFT, JEC_BITS))
    }

    /// The number of threads that are not actively
    /// executing work. They may be idle, sleepy, or asleep.
    pub(super) fn inactive_threads(self) -> usize {
        select_bits(self.word, INACTIVE_SHIFT, THREADS_BITS)
    }

    pub(super) fn awake_but_idle_threads(self) -> usize {
        debug_assert!(
            self.sleeping_threads() <= self.inactive_threads(),
            "sleeping threads: {} > raw idle threads {}",
            self.sleeping_threads(),
            self.inactive_threads()
        );
        self.inactive_threads() - self.sleeping_threads()
    }

    pub(super) fn sleeping_threads(self) -> usize {
        select_bits(self.word, SLEEPING_SHIFT, THREADS_BITS)
    }
}

impl std::fmt::Debug for Counters {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let word = format!("{:016x}", self.word);
        fmt.debug_struct("Counters")
            .field("word", &word)
            .field("jobs", &self.jobs_counter().0)
            .field("inactive", &self.inactive_threads())
            .field("sleeping", &self.sleeping_threads())
            .finish()
    }
}
