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
    JobCompletedOk,
    JobPanickedErrorStored,
    JobPanickedErrorNotStored,
    ScopeCompletePanicked,
    ScopeCompleteNoPanic,
}

pub const DUMP_LOGS: bool = cfg!(debug_assertions);

lazy_static! {
    pub static ref LOG_ENV: bool = env::var("RAYON_LOG").is_ok() || env::var("RAYON_RS_LOG").is_ok();
}

#[cfg(feature = "debug")]
lazy_static! {
    pub static ref FIBER_LOG_ENV: bool = env::var("RAYON_FIBER_LOG").is_ok();
}

macro_rules! log {
    ($event:expr) => {
        if ::log::DUMP_LOGS { if *::log::LOG_ENV { debug_log!("{:?}", $event); } }
    }
}

macro_rules! debug_log {
    ($($args:tt)*) => {{
        let str = &format!($($args)*);
        println!("{}", str);
        #[cfg(windows)]
        #[allow(unused_unsafe)]
        {
            let str = &format!("{}\n\x00", str)[..];
            unsafe {
                ::kernel32::OutputDebugStringA(str as *const str as _);
            }
        }
    }}
}

macro_rules! fiber_log {
    ($($args:tt)*) => {{
        #[cfg(feature = "debug")]
        #[allow(unused_unsafe)]
        {if *::log::FIBER_LOG_ENV {
            use registry::WorkerThread;;
            if WorkerThread::current() == 0i32 as _ {
                debug_log!("outside pool :: {}", &format!($($args)*));
            } else {
                let worker = unsafe { &*WorkerThread::current() };
                debug_log!("fiber {:?} - worker {} :: {}", worker.fiber_info().id, worker.index(), &format!($($args)*));
            }
        }}
    }}
}
