const USAGE: &'static str = "
Usage: sieve bench
       sieve --help

Sieve of Eratosthenes

A sieve finds prime numbers by first assuming all candidates are prime,
then progressively using small primes to mark their multiples composite.

Note that a given prime p only needs to start marking its multiples >=p²,
as anything less will already have been marked by an even smaller prime
factor.  This also means that sieving up to N only requires primes up to
sqrt(N) - a useful fact to prepare for parallel sieving.

Here we have three forms of the sieve:

- `sieve_serial`: direct iteration in serial
- `sieve_chunks`: chunked iteration, still serial
- `sieve_parallel`: chunked iteration in parallel

The performance improvement of `sieve_chunks` is largely due to cache
locality.  Since the sieve has to sweep repeatedly over the same memory,
it's helpful that the chunk size can fit entirely in the CPU cache.  Then
`sieve_parallel` also benefits from this as long as there's cache room for
multiple chunks, for the separate jobs in each thread.

Commands:
    bench              Run the benchmark in different modes and print the timings.

Options:
    -h, --help         Show this message.
";

#[derive(RustcDecodable)]
pub struct Args {
    cmd_bench: bool,
}

#[cfg(test)]
mod bench;

use docopt::Docopt;
use odds::stride::StrideMut;
use rayon::prelude::*;
use time;

const CHUNK_SIZE: usize = 100_000;

// Number of Primes < 10^n
// https://oeis.org/A006880
const NUM_PRIMES: &'static [usize] = &[
    0, // primes in 0..10^0
    4, // primes in 0..10^1
    25, // etc
    168,
    1229,
    9592,
    78498,
    664579,
    5761455,
    50847534,
    455052511,
    4118054813,
    37607912018,
    346065536839,
    3204941750802,
    29844570422669,
    279238341033925,
    2623557157654233,
    24739954287740860,
    234057667276344607,
    2220819602560918840,
];

// For all of these sieves, sieve[i]==true -> 2*i+1 is prime

fn max(magnitude: usize) -> usize {
    10_usize.pow(magnitude as u32)
}

/// Sieve odd integers for primes < max.
fn sieve_serial(max: usize) -> Vec<bool> {
    let mut sieve = vec![true; max / 2];
    sieve[0] = false; // 1 is not prime
    for i in 1.. {
        if sieve[i] {
            let p = 2 * i + 1;
            let pp = p * p;
            if pp >= max {
                break;
            }
            clear_stride(&mut sieve, pp / 2, p);
        }
    }
    sieve
}

/// Sieve odd integers for primes < max using chunks.
fn sieve_chunks(max: usize) -> Vec<bool> {
    // first compute the small primes, up to sqrt(max).
    let small_max = (max as f64).sqrt().ceil() as usize;
    let mut sieve = sieve_serial(small_max);
    sieve.resize(max / 2, true);

    {
        let (low, high) = sieve.split_at_mut(small_max / 2);
        for (chunk_index, chunk) in high.chunks_mut(CHUNK_SIZE).enumerate() {
            let i = small_max / 2 + chunk_index * CHUNK_SIZE;
            let base = i * 2 + 1;
            update_chunk(low, chunk, base);
        }
    }

    sieve
}

/// Sieve odd integers for primes < max, in parallel!
fn sieve_parallel(max: usize) -> Vec<bool> {
    // first compute the small primes, up to sqrt(max).
    let small_max = (max as f64).sqrt().ceil() as usize;
    let mut sieve = sieve_serial(small_max);
    sieve.resize(max / 2, true);

    {
        let (low, high) = sieve.split_at_mut(small_max / 2);
        high.par_chunks_mut(CHUNK_SIZE)
            .enumerate() // to figure out where this chunk came from
            .with_max_len(1) // ensure every single chunk is a separate rayon job
            .for_each(|(chunk_index, chunk)| {
                let i = small_max / 2 + chunk_index * CHUNK_SIZE;
                let base = i * 2 + 1;
                update_chunk(low, chunk, base);
            });
    }

    sieve
}

/// Update a chunk with low primes
fn update_chunk(low: &[bool], chunk: &mut [bool], base: usize) {
    let max = base + chunk.len() * 2;
    for (i, &is_prime) in low.iter().enumerate() {
        if is_prime {
            let p = 2 * i + 1;
            let pp = p * p;
            if pp >= max {
                break;
            }

            let pm = if pp < base {
                // p² is too small - find the first odd multiple that's in range
                ((base + p - 1) / p | 1) * p
            } else {
                pp
            };

            if pm < max {
                clear_stride(chunk, (pm - base) / 2, p);
            }
        }
    }
}

fn clear_stride(slice: &mut [bool], from: usize, stride: usize) {
    let slice = &mut slice[from..];
    for x in StrideMut::from_slice(slice, stride as isize) {
        *x = false;
    }
}

fn measure(f: fn(usize) -> Vec<bool>) -> u64 {
    const MAGNITUDE: usize = 9;

    let start = time::precise_time_ns();
    let sieve = f(max(MAGNITUDE));
    let duration = time::precise_time_ns() - start;

    // sanity check the number of primes found
    let num_primes = 1 + sieve.into_iter().filter(|&b| b).count();
    assert_eq!(num_primes, NUM_PRIMES[MAGNITUDE]);

    duration
}

pub fn main(args: &[String]) {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(args).decode())
        .unwrap_or_else(|e| e.exit());

    if args.cmd_bench {
        let serial = measure(sieve_serial);
        println!("  serial: {:10} ns", serial);

        let chunks = measure(sieve_chunks);
        println!(
            "  chunks: {:10} ns -> {:.2}x speedup",
            chunks,
            serial as f64 / chunks as f64
        );

        let parallel = measure(sieve_parallel);
        println!(
            "parallel: {:10} ns -> {:.2}x speedup",
            parallel,
            chunks as f64 / parallel as f64
        );
    }
}
