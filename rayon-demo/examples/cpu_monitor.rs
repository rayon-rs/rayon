use clap::{Parser, Subcommand};
use std::io;

const ABOUT: &str = "
A test for monitoring how much CPU usage Rayon consumes under various
scenarios. This test is intended to be executed interactively, like so:

    cargo run --example cpu_monitor -- tasks-ended
";

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// After all tasks have finished, go to sleep
    TasksEnded,
    /// A root task stalls for a very long time
    TaskStallRoot,
    /// A task in a scope stalls for a very long time
    TaskStallScope,
}

#[derive(Parser, Debug)]
#[clap(about = ABOUT)]
pub struct Args {
    #[clap(subcommand)]
    command: Commands,

    /// Control how hard the dummy task works
    #[clap(short = 'd', long, default_value_t = 27)]
    depth: usize,
}

fn main() {
    let args: Args = Args::from_args();
    match args.command {
        Commands::TasksEnded => tasks_ended(&args),
        Commands::TaskStallRoot => task_stall_root(&args),
        Commands::TaskStallScope => task_stall_scope(&args),
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

    println!("Starting heavy work at depth {}...wait.", args.depth);
    join_recursively(args.depth);
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
