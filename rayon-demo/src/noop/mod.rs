use crate::cpu_time;
use clap::Parser;

#[derive(Parser)]
#[clap(
    author,
    version,
    about = "Noop loop to measure CPU usage. See rayon-rs/rayon#642."
)]
pub struct Args {
    /// How long to sleep (in millis) between doing a spawn.
    #[clap(long, default_value_t = 10)]
    sleep: u64,

    /// Total time to execution (in millis).
    #[clap(long, default_value_t = 100)]
    iters: u64,
}

pub fn main(args: &[String]) {
    let args: Args = Parser::parse_from(args);

    let m = cpu_time::measure_cpu(|| {
        for _ in 1..args.iters {
            std::thread::sleep(std::time::Duration::from_millis(args.sleep));
            rayon::spawn(move || {});
        }
    });
    println!("noop --iters={} --sleep={}", args.iters, args.sleep);
    cpu_time::print_time(m);
}
