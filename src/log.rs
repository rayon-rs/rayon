//! Debug Logging

use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT};

#[derive(Debug)]
pub enum Event {
    StartWorking { index: usize },
    WaitForWork { worker: usize, was_active: bool },
    FoundWork { worker: usize },
    StoleWork { worker: usize, victim: usize },
    WaitUntil { worker: usize },
    LatchSet { worker: usize },
    DidNotFindWork { worker: usize, },
    InjectJobs { count: usize },
    Join { worker: usize },
    PoppedJob { worker: usize },
    PoppedRhs { worker: usize },
    LostJob { worker: usize },
}

pub const DUMP_LOGS: bool = false;

macro_rules! log {
    ($event:expr) => {
        if ::log::DUMP_LOGS { println!("{:?}", $event); }
    }
}

pub static STOLEN_JOB: AtomicUsize = ATOMIC_USIZE_INIT;

macro_rules! stat_stolen {
    () => {
        ::log::STOLEN_JOB.fetch_add(1, ::std::sync::atomic::Ordering::SeqCst);
    }
}

pub static POPPED_JOB: AtomicUsize = ATOMIC_USIZE_INIT;

macro_rules! stat_popped {
    () => {
        ::log::POPPED_JOB.fetch_add(1, ::std::sync::atomic::Ordering::SeqCst);
    }
}

macro_rules! dump_stats {
    () => {
        {
            let stolen = ::log::STOLEN_JOB.load(::std::sync::atomic::Ordering::SeqCst);
            println!("Jobs stolen: {:?}", stolen);
            let popped = ::log::POPPED_JOB.load(::std::sync::atomic::Ordering::SeqCst);
            println!("Jobs popped: {:?}", popped);
        }
    }
}
