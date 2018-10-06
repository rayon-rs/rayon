use time::Duration;

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use self::win::get_cpu_time;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use self::unix::get_cpu_time;

#[cfg(not(any(unix, windows)))]
pub fn get_cpu_time() -> Option<u64> {
    None
}

pub fn get_cpu_duration(start: Option<u64>, stop: Option<u64>) -> Option<Duration> {
    start.and_then(|start| stop.and_then(|stop| Some(Duration::nanoseconds((stop - start) as i64))))
}
