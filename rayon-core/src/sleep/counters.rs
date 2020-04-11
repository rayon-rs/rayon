use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct AtomicCounters {
    /// Packs together a number of counters.
    ///
    /// Consider the value `0x1111_2222_3333_4444`:
    ///
    /// - The bits 0x1111 are the **jobs event counter**.
    /// - The bits 0x2222 are the **sleepy event counter**.
    /// - The bits 0x3333 are the number of **idle threads**.
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
pub(super) struct JobsEventCounter(u16);

impl JobsEventCounter {
    pub(super) const INVALID: JobsEventCounter = JobsEventCounter(JOBS_MAX);

    fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// Returns true if there were sleepy workers pending when this reading was
    /// taken. This is determined by checking whether the value is odd (sleepy
    /// workers) or even (work published since last sleepy worker got sleepy,
    /// though they may not have seen it yet).
    pub(super) fn is_sleepy(self) -> bool {
        (self.as_usize() & 1) != 0
    }
}

const ONE_SLEEPING: u64 = 0x0000_0000_0000_0001;
const ONE_IDLE: u64 = 0x0000_0000_0001_0000;
const ONE_JEC: u64 = 0x0000_0001_0000_0000;
const SLEEPING_SHIFT: u64 = 0;
const IDLE_SHIFT: u64 = 1 * 16;
const JOBS_SHIFT: u64 = 2 * 16;
const JOBS_MAX: u16 = 0xFFFF;

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

    /// Adds an idle thread. This cannot fail.
    #[inline]
    pub(super) fn add_idle_thread(&self) {
        self.value.fetch_add(ONE_IDLE, Ordering::SeqCst);
    }

    /// Attempts to increment the jobs event counter by one, returning true
    /// if it succeeded. This can be used for two purposes:
    ///
    /// * If a thread is getting sleepy, and the JEC is even, then it will attempt
    ///   to increment to an odd value.
    /// * If a thread is publishing work, and the JEC is odd, then it will attempt
    ///   to increment to an event value.
    pub(super) fn try_increment_jobs_event_counter(&self, old_value: Counters) -> bool {
        let new_value = Counters::new(old_value.word + ONE_JEC);
        self.try_exchange(old_value, new_value, Ordering::SeqCst)
    }

    /// Subtracts an idle thread. This cannot fail; it returns the
    /// number of sleeping threads to wake up (if any).
    pub(super) fn sub_idle_thread(&self) -> u16 {
        let old_value = Counters::new(self.value.fetch_sub(ONE_IDLE, Ordering::SeqCst));
        debug_assert!(
            old_value.raw_idle_threads() > 0,
            "sub_idle_thread: old_value {:?} has no idle threads",
            old_value,
        );

        // Current heuristic: whenever an idle thread goes away, if
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
    }

    #[inline]
    pub(super) fn try_add_sleeping_thread(&self, old_value: Counters) -> bool {
        debug_assert!(
            old_value.raw_idle_threads() > 0,
            "try_add_sleeping_thread: old_value {:?} has no idle threads",
            old_value,
        );
        debug_assert!(
            old_value.sleeping_threads() < std::u16::MAX,
            "try_add_sleeping_thread: old_value {:?} has too many sleeping threads",
            old_value,
        );

        let mut new_value = old_value;
        new_value.word += ONE_SLEEPING;

        self.try_exchange(old_value, new_value, Ordering::SeqCst)
    }
}

fn select_u16(word: u64, shift: u64) -> u16 {
    (word >> shift) as u16
}

impl Counters {
    fn new(word: u64) -> Counters {
        Counters { word }
    }

    pub(super) fn jobs_counter(self) -> JobsEventCounter {
        JobsEventCounter(select_u16(self.word, JOBS_SHIFT))
    }

    pub(super) fn raw_idle_threads(self) -> u16 {
        select_u16(self.word, IDLE_SHIFT)
    }

    pub(super) fn awake_but_idle_threads(self) -> u16 {
        self.raw_idle_threads() - self.sleeping_threads()
    }

    pub(super) fn sleeping_threads(self) -> u16 {
        select_u16(self.word, SLEEPING_SHIFT)
    }
}

impl std::fmt::Debug for Counters {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let word = format!("{:016x}", self.word);
        fmt.debug_struct("Counters")
            .field("word", &word)
            .field("jobs", &self.jobs_counter().0)
            .field("idle", &self.raw_idle_threads())
            .field("sleeping", &self.sleeping_threads())
            .finish()
    }
}
