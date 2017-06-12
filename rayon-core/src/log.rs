//! Debug Logging
//!
//! To use in a debug build, set the env var `RAYON_LOG=1`.  In a
//! release build, logs are compiled out. You will have to change
//! `DUMP_LOGS` to be `true`.
//!
//! **Old environment variable:** `RAYON_LOG` is a one-to-one
//! replacement of the now deprecated `RAYON_RS_LOG` environment
//! variable, which is still supported for backwards compatibility.

use std::env;

#[derive(Debug)]
pub enum Event {
    Tickle { worker: usize, old_state: usize },
    GetSleepy { worker: usize, state: usize },
    GotSleepy { worker: usize, old_state: usize, new_state: usize },
    GotAwoken { worker: usize },
    FellAsleep { worker: usize },
    GotInterrupted { worker: usize },
    FoundWork { worker: usize, yields: usize },
    DidNotFindWork { worker: usize, yields: usize },
    StoleWork { worker: usize, victim: usize },
    UninjectedWork { worker: usize },
    WaitUntil { worker: usize },
    LatchSet { worker: usize },
    InjectJobs { count: usize },
    Join { worker: usize },
    PoppedJob { worker: usize },
    PoppedRhs { worker: usize },
    LostJob { worker: usize },
    JobCompletedOk { owner_thread: usize },
    JobPanickedErrorStored { owner_thread: usize },
    JobPanickedErrorNotStored { owner_thread: usize },
    ScopeCompletePanicked { owner_thread: usize },
    ScopeCompleteNoPanic { owner_thread: usize },

    FutureExecute { state: usize },
    FutureExecuteReady,
    FutureExecuteNotReady,
    FutureExecuteErr,
    FutureInstallWaitingTask { state: usize },
    FutureUnparkWaitingTask,
    FutureComplete,
}

pub const DUMP_LOGS: bool = cfg!(debug_assertions);

lazy_static! {
    pub static ref LOG_ENV: bool = env::var("RAYON_LOG").is_ok() || env::var("RAYON_RS_LOG").is_ok();
}

macro_rules! log {
    ($event:expr) => {
        if ::log::DUMP_LOGS { if *::log::LOG_ENV { println!("{:?}", $event); } }
    }
}
