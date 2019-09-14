use libc::{getrusage, rusage, RUSAGE_SELF};
use std::mem;

pub fn get_cpu_time() -> Option<u64> {
    unsafe {
        let mut usage: rusage = mem::uninitialized();
        getrusage(RUSAGE_SELF, &mut usage);
        let user = 1_000_000_000 * (usage.ru_utime.tv_sec as u64)
            + 1_000 * (usage.ru_utime.tv_usec as u64);
        let system = 1_000_000_000 * (usage.ru_stime.tv_sec as u64)
            + 1_000 * (usage.ru_stime.tv_usec as u64);
        Some(user + system)
    }
}
