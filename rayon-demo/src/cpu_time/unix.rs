use libc::{getrusage, RUSAGE_SELF};
use std::mem::MaybeUninit;

pub fn get_cpu_time() -> Option<u64> {
    let usage = unsafe {
        let mut usage = MaybeUninit::uninit();
        if getrusage(RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return None;
        }
        usage.assume_init()
    };
    let user =
        1_000_000_000 * (usage.ru_utime.tv_sec as u64) + 1_000 * (usage.ru_utime.tv_usec as u64);
    let system =
        1_000_000_000 * (usage.ru_stime.tv_sec as u64) + 1_000 * (usage.ru_stime.tv_usec as u64);
    Some(user + system)
}
