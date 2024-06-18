use std::sync::{Arc, Mutex};
use rayon::ThreadPoolBuilder;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

fn mutex_and_par(mutex: Arc<Mutex<Vec<i32>>>, blocking_pool: &rayon::ThreadPool) {
    // Lock the mutex and collect items using the custom thread pool
    let vec = mutex.lock().unwrap();
    let result: Vec<i32> = blocking_pool.install(|| vec.par_iter().cloned().collect());
    println!("{:?}", result);
}

#[test]
#[cfg_attr(any(target_os = "emscripten", target_family = "wasm"), ignore)]
fn test_issue592() {
    // Initialize a collection and a mutex
    let collection = vec![1, 2, 3, 4, 5];
    let mutex = Arc::new(Mutex::new(collection));

    // Create a custom thread pool for the test
    let blocking_pool = ThreadPoolBuilder::new().full_blocking().num_threads(4).build().unwrap();

    // Call the function with the custom thread pool within a parallel iterator's for_each method
    let dummy_collection: Vec<i32> = (1..=100).collect();
    dummy_collection.par_iter().for_each(|_| {
        mutex_and_par(mutex.clone(), &blocking_pool);
    });

    // Additional assertions can be added here to verify the behavior
}

