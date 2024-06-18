use std::sync::{Arc, Mutex};
use rayon::ThreadPoolBuilder;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

fn mutex_and_par(mutex: Arc<Mutex<Vec<i32>>>, blocking_pool: &rayon::ThreadPool) {
    // Lock the mutex and collect items using the full blocking thread pool
    let vec = mutex.lock().unwrap();
    let result: Vec<i32> = blocking_pool.install(|| vec.par_iter().cloned().collect());
    println!("{:?}", result);
}

#[test]
fn test_issue592() {
    let collection = vec![1, 2, 3, 4, 5];
    let mutex = Arc::new(Mutex::new(collection));

    let blocking_pool = ThreadPoolBuilder::new().full_blocking().num_threads(4).build().unwrap();

    let dummy_collection: Vec<i32> = (1..=100).collect();
    dummy_collection.par_iter().for_each(|_| {
        mutex_and_par(mutex.clone(), &blocking_pool);
    });
}

