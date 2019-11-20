use std::mem::MaybeUninit;
use winapi::um::processthreadsapi::{GetCurrentProcess, GetProcessTimes};

pub fn get_cpu_time() -> Option<u64> {
    let (kernel, user) = unsafe {
        let process = GetCurrentProcess();
        let mut _creation = MaybeUninit::uninit();
        let mut _exit = MaybeUninit::uninit();
        let mut kernel = MaybeUninit::uninit();
        let mut user = MaybeUninit::uninit();
        if GetProcessTimes(
            process,
            _creation.as_mut_ptr(),
            _exit.as_mut_ptr(),
            kernel.as_mut_ptr(),
            user.as_mut_ptr(),
        ) == 0
        {
            return None;
        }
        (kernel.assume_init(), user.assume_init())
    };
    let kernel = (kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64;
    let user = (user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64;
    Some(100 * (kernel + user))
}
