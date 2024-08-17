use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rayon::prelude::ParallelSliceMut;


#[test]
fn flat_map_test() {
    let num_iterations: Vec<usize> = (0..10_000).collect();

    let data: Vec<usize> = num_iterations.par_iter().flat_map(|x| [100 * x + 0, 100 * x + 1, 100 * x + 2, 100 * x + 3]).collect();

    assert_eq!(data.len(), num_iterations.len() * 4);

    for (i, value) in data.into_iter().enumerate() {
        assert_eq!(value, (i / 4) * 100 + (i % 4));
    }
}

#[test]
fn flat_map_memory() {
    let num_iterations: Vec<usize> = (0..10_000*10_000).collect();

    let data: Vec<usize> = num_iterations.par_iter().flat_map(|x| [100 * x + 0, 100 * x + 1, 100 * x + 2, 100 * x + 3]).collect();

    assert_eq!(data.len(), num_iterations.len() * 4);

    for (i, value) in data.into_iter().enumerate() {
        assert_eq!(value, (i / 4) * 100 + (i % 4));
    }
}


const LEN: usize = 10_000 * 10_000;

// 0.7s
#[test]
fn theoretical_best() {
    let num_iterations = vec![0u8; LEN];

    let mut data = vec![0u8; num_iterations.len() * 4];
    data.par_chunks_mut(4).zip(num_iterations).for_each(|(data, _num_iterations)| {
        // fill in this chunk
        data[0] = 0;
        data[1] = 1;
        data[2] = 2;
        data[3] = 3;
    });

    assert_eq!(data.len(), LEN * 4);
}

// 31.12s
#[test]
fn flat_map_version() {
    let num_iterations = vec![0u8; LEN];

    let data: Vec<usize> = num_iterations.par_iter().flat_map(|_x| [0, 1, 2, 3]).collect();

    assert_eq!(data.len(), LEN * 4);
}


// 2.12s
#[test]
fn flat_map_iter_version() {
    let num_iterations = vec![0u8; LEN];

    let data: Vec<usize> = num_iterations.par_iter().flat_map_iter(|_x| [0, 1, 2, 3]).collect();

    assert_eq!(data.len(), LEN * 4);
}
