use std::io;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum ArgScenario {
    /// after all tasks have finished, go to sleep
    TaskEnded,
    /// a root task stalls for a very long time
    TaskStallRoot,
    /// a task in a scope stalls for a very long time
    TaskStallScope,
}

#[derive(StructOpt, Debug)]
#[structopt(
    about = "A test for monitoring how much CPU usage Rayon consumes under various scenarios."
)]
pub struct Args {
    #[structopt(subcommand)]
    arg_scenario: ArgScenario,
    /// Control how hard the dummy task works
    #[structopt(short = "d", long = "depth", default_value = "27")]
    flag_depth: usize,
}

fn main() {
    use ArgScenario::*;
    let args: Args = Args::from_args();

    match args.arg_scenario {
        TaskEnded => tasks_ended(&args),
        TaskStallRoot => task_stall_root(&args),
        TaskStallScope => task_stall_scope(&args),
    }
}

fn wait_for_user() {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
}

fn task(args: &Args) {
    fn join_recursively(n: usize) {
        if n == 0 {
            return;
        }
        rayon::join(|| join_recursively(n - 1), || join_recursively(n - 1));
    }

    println!("Starting heavy work at depth {}...wait.", args.flag_depth);
    join_recursively(args.flag_depth);
    println!("Heavy work done; check top. You should see CPU usage drop to zero soon.");
    println!("Press <enter> to quit...");
}

fn tasks_ended(args: &Args) {
    task(args);
    wait_for_user();
}

fn task_stall_root(args: &Args) {
    rayon::join(|| task(args), wait_for_user);
}

fn task_stall_scope(args: &Args) {
    rayon::scope(|scope| {
        scope.spawn(move |_| task(args));
        scope.spawn(move |_| wait_for_user());
    });
}
