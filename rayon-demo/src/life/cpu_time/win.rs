use std::mem;
use winapi::shared::minwindef::FILETIME;
use winapi::um::processthreadsapi::{GetCurrentProcess, GetProcessTimes};

pub fn get_cpu_time() -> Option<u64> {
    unsafe {
        let process = GetCurrentProcess();
        let mut _creation: FILETIME = mem::uninitialized();
        let mut _exit: FILETIME = mem::uninitialized();
        let mut kernel: FILETIME = mem::uninitialized();
        let mut user: FILETIME = mem::uninitialized();
        GetProcessTimes(process, &mut _creation, &mut _exit, &mut kernel, &mut user);
        let kernel = (kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64;
        let user = (user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64;
        Some(100 * (kernel + user))
    }
}
