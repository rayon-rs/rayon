//! Debug Logging
//!
//! To use in a debug build, set the env var `RAYON_LOG=1`.  In a
//! release build, logs are compiled out. You will have to change
//! `DUMP_LOGS` to be `true`.
//!
//! **Old environment variable:** `RAYON_LOG` is a one-to-one
//! replacement of the now deprecated `RAYON_RS_LOG` environment
//! variable, which is still supported for backwards compatibility.

#[cfg(debug_assertions)]
use std::env;

#[cfg_attr(debug_assertions, derive(Debug))]
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub(super) enum Event {
    Tickle {
        worker: usize,
        old_state: usize,
    },
    GetSleepy {
        worker: usize,
        state: usize,
    },
    GotSleepy {
        worker: usize,
        old_state: usize,
        new_state: usize,
    },
    GotAwoken {
        worker: usize,
    },
    FellAsleep {
        worker: usize,
    },
    GotInterrupted {
        worker: usize,
    },
    FoundWork {
        worker: usize,
        yields: usize,
    },
    DidNotFindWork {
        worker: usize,
        yields: usize,
    },
    StoleWork {
        worker: usize,
        victim: usize,
    },
    UninjectedWork {
        worker: usize,
    },
    WaitUntil {
        worker: usize,
    },
    LatchSet {
        worker: usize,
    },
    InjectJobs {
        count: usize,
    },
    Join {
        worker: usize,
    },
    PoppedJob {
        worker: usize,
    },
    PoppedRhs {
        worker: usize,
    },
    LostJob {
        worker: usize,
    },
    JobCompletedOk {
        owner_thread: usize,
    },
    JobPanickedErrorStored {
        owner_thread: usize,
    },
    JobPanickedErrorNotStored {
        owner_thread: usize,
    },
    ScopeCompletePanicked {
        owner_thread: usize,
    },
    ScopeCompleteNoPanic {
        owner_thread: usize,
    },
}

#[cfg(debug_assertions)]
lazy_static::lazy_static! {
    pub(super) static ref LOG_ENV: bool =
        env::var("RAYON_LOG").is_ok() || env::var("RAYON_RS_LOG").is_ok();
}

#[cfg(debug_assertions)]
macro_rules! log {
    ($event:expr) => {
        if *$crate::log::LOG_ENV {
            eprintln!("{:?}", $event);
        }
    };
}

#[cfg(not(debug_assertions))]
macro_rules! log {
    ($event:expr) => {
        if false {
            // Expand `$event` so it still appears used, but without
            // any of the formatting code to be optimized away.
            $event;
        }
    };
}
