use time::{self, Duration};

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
    Some(Duration::nanoseconds((stop? - start?) as i64))
}

#[derive(Copy, Clone)]
pub struct CpuMeasure {
    /// number of ns
    pub time_duration: u64,

    /// percentage (0-100) of that as cpu time
    pub cpu_usage_percent: Option<f64>,
}

pub fn measure_cpu(op: impl FnOnce()) -> CpuMeasure {
    let time_start = time::precise_time_ns();
    let cpu_start = get_cpu_time();

    op();

    let cpu_stop = get_cpu_time();
    let time_duration = time::precise_time_ns() - time_start;

    CpuMeasure {
        time_duration,
        cpu_usage_percent: get_cpu_duration(cpu_start, cpu_stop)
            .and_then(|cpu| cpu.num_nanoseconds())
            .map(|cpu| 100.0 * cpu as f64 / time_duration as f64),
    }
}

pub fn print_time(m: CpuMeasure) {
    println!("    wallclock: {} ns", m.time_duration);
    if let Some(cpu_usage) = m.cpu_usage_percent {
        println!("    cpu usage: {:3.1}%", cpu_usage);
    } else {
        println!("    cpu usage: N/A");
    }
}
