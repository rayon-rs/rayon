extern crate rayon;

use std::collections::HashSet;

use rayon::*;
use rayon::prelude::*;

fn main() {
    let result = initialize(Configuration::new().thread_name(|i| format!("hello-name-test-{}", i)));

    const N: usize = 10000;

    let thread_names = (0..N).into_par_iter()
        .flat_map(|_| ::std::thread::current().name().map(|s| s.to_owned()))
        .collect::<HashSet<String>>();

    let all_contains_name = thread_names.iter().all(|name| name.starts_with("hello-name-test-"));
    assert!(all_contains_name);
}
