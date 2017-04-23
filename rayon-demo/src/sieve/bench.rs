use test;

use super::NUM_PRIMES;

const MAGNITUDE: usize = 7;

fn sieve_bench<TICK>(b: &mut test::Bencher, mut tick: TICK)
where
    TICK: FnMut(usize) -> Vec<bool>,
{
    let mut result = vec![];
    b.iter(|| result = tick(super::max(MAGNITUDE)));
    let num_primes = 1 + result.into_iter().filter(|&b| b).count();
    assert_eq!(num_primes, NUM_PRIMES[MAGNITUDE]);
}

#[bench]
fn sieve_serial(b: &mut ::test::Bencher) {
    sieve_bench(b, super::sieve_serial);
}

#[bench]
fn sieve_chunks(b: &mut ::test::Bencher) {
    sieve_bench(b, super::sieve_chunks);
}

#[bench]
fn sieve_parallel(b: &mut ::test::Bencher) {
    sieve_bench(b, super::sieve_parallel);
}
