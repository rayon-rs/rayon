//! Tick-shaped workload benchmark for the worker idle/sleep protocol.
//!
//! Models a control loop that runs several short parallel regions back to
//! back, separated by short sequential stretches, with an idle gap between
//! ticks: the shape of the reconcile and render loops of long-running
//! services, where a pool spends most of its life between bursts of work
//! and the cost of idle spinning and of wake-up latency both show. Vary
//! `BENCH_THREADS` to see how pool width affects tick latency and CPU on
//! these shapes.
//!
//! Two tick shapes are modelled, selected by `BENCH_MODE`:
//!
//! - `regions` (default): each parallel region is its own `install`
//!   from the driving thread, with sequential work between regions done
//!   outside the pool, so the pool is fully idle between regions.
//! - `install`: the whole tick is one `install`; the bursts of parallel
//!   work and the serial stretches between them run on the installing
//!   worker, so the pool is never idle during the tick and the other
//!   workers see only "a region in progress with nothing to steal"
//!   during each stretch. This is the shape of a coordinator's
//!   reconcile tick as traced in production: ~12 bursts of a few dozen
//!   microsecond-sized jobs separated by ~200 us serial stretches inside
//!   one ~3.5 ms install, then ~3 ms outside the pool.
//!
//! Configuration is via environment variables (all optional):
//!
//! | var                | default   | meaning                                        |
//! |--------------------|-----------|------------------------------------------------|
//! | `BENCH_MODE`       | regions   | regions \| install (see above)                 |
//! | `BENCH_THREADS`    | ncpu/2    | pool width                                     |
//! | `BENCH_TICKS`      | 2000      | number of ticks measured                       |
//! | `BENCH_REGIONS`    | 8         | parallel regions (bursts) per tick             |
//! | `BENCH_LEAVES`     | 64        | leaf tasks per region (split by nested join)   |
//! | `BENCH_LEAF_US`    | 10        | busy work per leaf task, microseconds          |
//! | `BENCH_GAP_US`     | 20        | sequential work between regions, microseconds  |
//! | `BENCH_TICK_MS`    | 10        | tick period (idle gap = period - tick work)    |
//!
//! Output: one line of tab-separated fields (mode, threads, tick p50/p99,
//! region p50/p99, process CPU in cores over the measured window).
//!
//! ```text
//! cargo run --release -p rayon-core --example tick_bench
//! BENCH_THREADS=8 BENCH_MODE=install cargo run --release -p rayon-core --example tick_bench
//! ```

use std::hint::black_box;
use std::time::{Duration, Instant};

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Burn roughly `us` microseconds of CPU without touching memory.
fn busy(us: u64) {
    let end = Instant::now() + Duration::from_micros(us);
    let mut x = 0u64;
    while Instant::now() < end {
        for _ in 0..64 {
            x = black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
    }
}

/// A parallel region: `leaves` leaf tasks reached by recursive `join`,
/// the way `par_iter` and nested `par_join!` split their work.
fn region(leaves: usize, leaf_us: u64) {
    if leaves <= 1 {
        busy(leaf_us);
    } else {
        let half = leaves / 2;
        rayon_core::join(|| region(half, leaf_us), || region(leaves - half, leaf_us));
    }
}

/// utime + stime of this process, in seconds (Linux).
fn proc_cpu_seconds() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // Fields after the parenthesised comm: index 13/14 (0-based from field 0)
    // are utime/stime in clock ticks.
    let rest = stat.rsplit(')').next().unwrap_or("");
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ticks: u64 = f.get(11).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0)
        + f.get(12).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    ticks as f64 / 100.0
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn main() {
    let mode: String = env_or("BENCH_MODE", "regions".to_string());
    let install_mode = match mode.as_str() {
        "regions" => false,
        "install" => true,
        other => panic!("BENCH_MODE={other}: expected regions|install"),
    };
    let ncpu = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    let threads: usize = env_or("BENCH_THREADS", (ncpu / 2).max(1));
    let ticks: usize = env_or("BENCH_TICKS", 2000);
    let regions: usize = env_or("BENCH_REGIONS", 8);
    let leaves: usize = env_or("BENCH_LEAVES", 64);
    let leaf_us: u64 = env_or("BENCH_LEAF_US", 10);
    let gap_us: u64 = env_or("BENCH_GAP_US", 20);
    let tick_ms: u64 = env_or("BENCH_TICK_MS", 10);

    let pool = rayon_core::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();

    // Warm up: get every worker created and scheduled once.
    for _ in 0..50 {
        pool.install(|| region(leaves, leaf_us));
        std::thread::sleep(Duration::from_millis(tick_ms));
    }

    let mut tick_lat = Vec::with_capacity(ticks);
    let mut region_lat = Vec::with_capacity(ticks * regions);
    let mut by_index: Vec<Vec<f64>> = vec![Vec::with_capacity(ticks); regions];
    let period = Duration::from_millis(tick_ms);
    let cpu0 = proc_cpu_seconds();
    let wall0 = Instant::now();
    let mut next = wall0;
    for _ in 0..ticks {
        next += period;
        let t0 = Instant::now();
        if install_mode {
            let lats = pool.install(|| {
                let mut lats = Vec::with_capacity(regions);
                for r in 0..regions {
                    if r > 0 {
                        busy(gap_us);
                    }
                    let r0 = Instant::now();
                    region(leaves, leaf_us);
                    lats.push(r0.elapsed().as_secs_f64() * 1e3);
                }
                lats
            });
            for (lat, ms) in by_index.iter_mut().zip(lats) {
                region_lat.push(ms);
                lat.push(ms);
            }
        } else {
            for (r, lat) in by_index.iter_mut().enumerate() {
                if r > 0 {
                    busy(gap_us);
                }
                let r0 = Instant::now();
                pool.install(|| region(leaves, leaf_us));
                let ms = r0.elapsed().as_secs_f64() * 1e3;
                region_lat.push(ms);
                lat.push(ms);
            }
        }
        tick_lat.push(t0.elapsed().as_secs_f64() * 1e3);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
    let wall = wall0.elapsed().as_secs_f64();
    let cpu = proc_cpu_seconds() - cpu0;
    if std::env::var_os("BENCH_VERBOSE").is_some() {
        for (i, v) in by_index.iter_mut().enumerate() {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            eprintln!(
                "  region[{i}] p50={:.3}ms p90={:.3}ms p99={:.3}ms",
                percentile(v, 0.5),
                percentile(v, 0.9),
                percentile(v, 0.99)
            );
        }
    }
    tick_lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    region_lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Ideal region time with perfect parallelism, for reference.
    let ideal_region_ms = (leaves as f64 * leaf_us as f64 / threads as f64).ceil() / 1e3;
    println!(
        "mode={mode}\tthreads={threads}\tregions={regions}\tleaves={leaves}\tleaf_us={leaf_us}\tgap_us={gap_us}\ttick_ms={tick_ms}\t\
         tick_p50_ms={:.3}\ttick_p99_ms={:.3}\tregion_p50_ms={:.3}\tregion_p99_ms={:.3}\tregion_ideal_ms={:.3}\tcpu_cores={:.2}",
        percentile(&tick_lat, 0.5),
        percentile(&tick_lat, 0.99),
        percentile(&region_lat, 0.5),
        percentile(&region_lat, 0.99),
        ideal_region_ms,
        cpu / wall,
    );
}
