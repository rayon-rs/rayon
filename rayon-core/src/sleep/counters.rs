use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct AtomicCounters {
    /// Packs together a number of counters.
    ///
    /// Consider the value `0x1111_2222_3333_4444`:
    ///
    /// - The bits 0x1111 are the **jobs counter**.
    /// - The bits 0x2222 are the **sleepy counter**.
    /// - The bits 0x3333 are the number of **idle threads**.
    /// - The bits 0x4444 are the number of **sleeping threads**.
    ///
    /// See the struct `Counters` below.
    value: AtomicU64
}

#[derive(Copy, Clone)]
pub(super) struct Counters {
    word: u64
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub(super) struct SleepyCounter(u16);

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub(super) struct JobsCounter(u16);

const ONE_SLEEPING: u64 = 0x0000_0000_0000_0001;
const ONE_IDLE: u64 = 0x0000_0000_0001_0000;
const ONE_SLEEPY: u64 = 0x0000_0001_0000_0000;
const SLEEPY_ROLLVER_MASK: u64 = 0x0000_0000_FFFF_FFFF;
const NO_JOBS_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const SLEEPING_SHIFT: u64 = 0;
const IDLE_SHIFT: u64 = 1 * 16;
const SLEEPY_SHIFT: u64 = 2 * 16;
const JOBS_SHIFT: u64 = 3 * 16;

pub(super) const INVALID_SLEEPY_COUNTER: SleepyCounter = SleepyCounter(std::u16::MAX);
pub(super) const ZERO_SLEEPY_COUNTER: SleepyCounter = SleepyCounter(0);

impl AtomicCounters {
    pub(super) fn new() -> AtomicCounters {
        AtomicCounters { value: AtomicU64::new(0) }
    }

    pub(super) fn load(&self, ordering: Ordering) -> Counters {
        Counters::new(self.value.load(ordering))
    }

    #[inline]
    fn try_exchange(&self, old_value: Counters, new_value: Counters, ordering: Ordering) -> bool {
        self.value.compare_exchange(
            old_value.word,
            new_value.word,
            ordering,
            Ordering::Relaxed,
        ).is_ok()
    }

    /// Adds an idle thread. This cannot fail.
    #[inline]
    pub(super) fn add_idle_thread(&self) {
        self.value.fetch_add(ONE_IDLE, Ordering::SeqCst);
    }

    #[inline]
    pub(super) fn try_add_sleepy_thread(&self, old_value: Counters) -> bool {
        debug_assert!(
            !old_value.sleepy_counter().is_max(),
            "try_add_sleepy_thread: old_value {:?} has max sleepy threads",
            old_value,
        );
        let new_value = Counters::new(old_value.word + ONE_SLEEPY);
        self.try_exchange(old_value, new_value, Ordering::SeqCst)
    }

    /// Subtracts an idle thread. This cannot fail; it returns the
    /// number of sleeping threads to wake up (if any).
    #[inline]
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

    #[inline]
    pub(super) fn try_replicate_sleepy_counter(&self, old_value: Counters) -> bool {
        let sc = old_value.sleepy_counter();

        // clear jobs counter
        let word_without_jc = old_value.word & NO_JOBS_MASK;

        // replace with sleepy counter
        let sc_shifted_to_jc = (sc.0 as u64) << JOBS_SHIFT;
        let new_value = Counters::new(word_without_jc | sc_shifted_to_jc);

        debug_assert!(new_value.sleepy_counter() == old_value.sleepy_counter());
        debug_assert!(new_value.jobs_counter() == old_value.sleepy_counter());

        self.try_exchange(old_value, new_value, Ordering::SeqCst)
    }

    #[inline]
    pub(super) fn try_rollover_jobs_and_sleepy_counters(&self, old_value: Counters) -> bool {
        let new_value = Counters::new(old_value.word & SLEEPY_ROLLVER_MASK);
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

    pub(super) fn jobs_counter(self) -> JobsCounter {
        JobsCounter(select_u16(self.word, JOBS_SHIFT))
    }

    pub(super) fn sleepy_counter(self) -> SleepyCounter {
        SleepyCounter(select_u16(self.word, SLEEPY_SHIFT))
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

impl SleepyCounter {
    pub(super) fn is_max(self) -> bool {
        self.0 == std::u16::MAX
    }

    pub(super) fn as_u16(self) -> u16 {
        self.0
    }
}

impl PartialOrd<JobsCounter> for SleepyCounter {
    fn partial_cmp(&self, other: &JobsCounter) -> Option<::std::cmp::Ordering> {
        PartialOrd::partial_cmp(&self.0, &other.0)
    }
}

impl PartialOrd<SleepyCounter> for JobsCounter {
    fn partial_cmp(&self, other: &SleepyCounter) -> Option<::std::cmp::Ordering> {
        PartialOrd::partial_cmp(&self.0, &other.0)
    }
}

impl PartialEq<JobsCounter> for SleepyCounter {
    fn eq(&self, other: &JobsCounter) -> bool {
        PartialEq::eq(&self.0, &other.0)
    }
}

impl PartialEq<SleepyCounter> for JobsCounter {
    fn eq(&self, other: &SleepyCounter) -> bool {
        PartialEq::eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Counters {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let word = format!("{:016x}", self.word);
        fmt.debug_struct("Counters")
            .field("word", &word)
            .field("jobs", &self.jobs_counter().0)
            .field("sleepy", &self.sleepy_counter())
            .field("idle", &self.raw_idle_threads())
            .field("sleeping", &self.sleeping_threads())
            .finish()
    }
}
