const USAGE: &str = "
Usage: noop [--sleep N] [--iters N]

Noop loop to measure CPU usage. See rayon-rs/rayon#642.

Options:
    --sleep N       How long to sleep (in millis) between doing a spawn. [default: 10]
    --iters N        Total time to execution (in millis). [default: 100]
";

use docopt::Docopt;

#[derive(Deserialize)]
pub struct Args {
    flag_sleep: u64,
    flag_iters: u64,
}

pub fn main(args: &[String]) {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(args).deserialize())
        .unwrap_or_else(|e| e.exit());

    for _ in 1..args.flag_iters {
        std::thread::sleep(std::time::Duration::from_millis(args.flag_sleep));
        rayon::spawn(move || {  } );
    }
}
