use rand::distributions::Standard;
use rand::Rng;

const USAGE: &str = "
Usage: mergesort bench [--size N]
       mergesort --help

Parallel mergesort: regular sorting with a parallel merge step.
O(n log n) time; O(n) extra space; critical path is O(log^3 n)

Algorithm described in: https://software.intel.com/en-us/articles/a-parallel-stable-sort-using-c11-for-tbb-cilk-plus-and-openmp

Commands:
    bench              Run the benchmark in different modes and print the timings.

Options:
    --size N           Number of 32-bit words to sort [default: 250000000] (1GB)
    -h, --help         Show this message.
";

#[derive(serde::Deserialize)]
pub struct Args {
    cmd_bench: bool,
    flag_size: usize,
}

use docopt::Docopt;

use std::cmp::max;
use std::time::Instant;

pub fn merge_sort<T: Ord + Send + Copy>(v: &mut [T]) {
    let n = v.len();
    let mut buf = Vec::with_capacity(n);
    // We always overwrite the buffer before reading to it, and letting rust
    // initialize it would increase the critical path to O(n).
    unsafe {
        buf.set_len(n);
    }
    rsort(v, &mut buf[..]);
}

// Values from manual tuning gigasort on one machine.
const SORT_CHUNK: usize = 32 * 1024;
const MERGE_CHUNK: usize = 64 * 1024;

// Sort src, possibly making use of identically sized buf.
fn rsort<T: Ord + Send + Copy>(src: &mut [T], buf: &mut [T]) {
    if src.len() <= SORT_CHUNK {
        src.sort();
        return;
    }

    // Sort each half into half of the buffer.
    let mid = src.len() / 2;
    let (bufa, bufb) = buf.split_at_mut(mid);
    {
        let (sa, sb) = src.split_at_mut(mid);
        rayon::join(|| rsort_into(sa, bufa), || rsort_into(sb, bufb));
    }

    // Merge the buffer halves back into the original.
    rmerge(bufa, bufb, src);
}

// Sort src, putting the result into dest.
fn rsort_into<T: Ord + Send + Copy>(src: &mut [T], dest: &mut [T]) {
    let mid = src.len() / 2;
    let (s1, s2) = src.split_at_mut(mid);
    {
        // Sort each half.
        let (d1, d2) = dest.split_at_mut(mid);
        rayon::join(|| rsort(s1, d1), || rsort(s2, d2));
    }

    // Merge the halves into dest.
    rmerge(s1, s2, dest);
}

// Merge sorted inputs a and b, putting result in dest.
//
// Note: `a` and `b` have type `&mut [T]` and not `&[T]` because we do
// not want to require a `T: Sync` bound. Using `&mut` references
// proves to the compiler that we are not sharing `a` and `b` across
// threads and thus we only need a `T: Send` bound.
fn rmerge<T: Ord + Send + Copy>(a: &mut [T], b: &mut [T], dest: &mut [T]) {
    // Swap so a is always longer.
    let (a, b) = if a.len() > b.len() { (a, b) } else { (b, a) };
    if dest.len() <= MERGE_CHUNK {
        seq_merge(a, b, dest);
        return;
    }

    // Find the middle element of the longer list, and
    // use binary search to find its location in the shorter list.
    let ma = a.len() / 2;
    let mb = match b.binary_search(&a[ma]) {
        Ok(i) => i,
        Err(i) => i,
    };

    let (a1, a2) = a.split_at_mut(ma);
    let (b1, b2) = b.split_at_mut(mb);
    let (d1, d2) = dest.split_at_mut(ma + mb);
    rayon::join(|| rmerge(a1, b1, d1), || rmerge(a2, b2, d2));
}

// Merges sorted a and b into sorted dest.
#[inline(never)]
fn seq_merge<T: Ord + Copy>(a: &[T], b: &[T], dest: &mut [T]) {
    if b.is_empty() {
        dest.copy_from_slice(a);
        return;
    }
    let biggest = max(*a.last().unwrap(), *b.last().unwrap());
    let mut ai = a.iter();
    let mut an = *ai.next().unwrap();
    let mut bi = b.iter();
    let mut bn = *bi.next().unwrap();
    for d in dest.iter_mut() {
        if an < bn {
            *d = an;
            an = match ai.next() {
                Some(x) => *x,
                None => biggest,
            }
        } else {
            *d = bn;
            bn = match bi.next() {
                Some(x) => *x,
                None => biggest,
            }
        }
    }
}

#[test]
fn test_merge_sort() {
    let mut v = vec![1; 200_000];
    merge_sort(&mut v[..]);

    let sorted: Vec<u32> = (1..1_000_000).collect();
    let mut v = sorted.clone();
    merge_sort(&mut v[..]);
    assert_eq!(sorted, v);

    v.reverse();
    merge_sort(&mut v[..]);
    assert_eq!(sorted, v);
}

pub fn seq_merge_sort<T: Ord + Copy>(v: &mut [T]) {
    let n = v.len();
    let mut buf = Vec::with_capacity(n);
    // We always overwrite the buffer before reading to it, and we want to
    // duplicate the behavior of parallel sort.
    unsafe {
        buf.set_len(n);
    }
    seq_sort(v, &mut buf[..]);
}

// Sort src, possibly making use of identically sized buf.
fn seq_sort<T: Ord + Copy>(src: &mut [T], buf: &mut [T]) {
    if src.len() <= SORT_CHUNK {
        src.sort();
        return;
    }

    // Sort each half into half of the buffer.
    let mid = src.len() / 2;
    let (bufa, bufb) = buf.split_at_mut(mid);
    {
        let (sa, sb) = src.split_at_mut(mid);
        seq_sort_into(sa, bufa);
        seq_sort_into(sb, bufb);
    }

    // Merge the buffer halves back into the original.
    seq_merge(bufa, bufb, src);
}

// Sort src, putting the result into dest.
fn seq_sort_into<T: Ord + Copy>(src: &mut [T], dest: &mut [T]) {
    let mid = src.len() / 2;
    let (s1, s2) = src.split_at_mut(mid);
    {
        // Sort each half.
        let (d1, d2) = dest.split_at_mut(mid);
        seq_sort(s1, d1);
        seq_sort(s2, d2);
    }

    // Merge the halves into dest.
    seq_merge(s1, s2, dest);
}

pub fn is_sorted<T: Send + Ord>(v: &mut [T]) -> bool {
    let n = v.len();
    if n <= SORT_CHUNK {
        for i in 1..n {
            if v[i - 1] > v[i] {
                return false;
            }
        }
        return true;
    }

    let mid = n / 2;
    if v[mid - 1] > v[mid] {
        return false;
    }
    let (a, b) = v.split_at_mut(mid);
    let (left, right) = rayon::join(|| is_sorted(a), || is_sorted(b));
    left && right
}

fn default_vec(n: usize) -> Vec<u32> {
    let rng = crate::seeded_rng();
    rng.sample_iter(&Standard).take(n).collect()
}

fn timed_sort<F: FnOnce(&mut [u32])>(n: usize, f: F, name: &str) -> u64 {
    let mut v = default_vec(n);

    let start = Instant::now();
    f(&mut v[..]);
    let dur = Instant::now() - start;
    let nanos = u64::from(dur.subsec_nanos()) + dur.as_secs() * 1_000_000_000u64;
    println!("{}: sorted {} ints: {} s", name, n, nanos as f32 / 1e9f32);

    // Check correctness
    assert!(is_sorted(&mut v[..]));

    nanos
}

pub fn main(args: &[String]) {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(args).deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.cmd_bench {
        let seq = timed_sort(args.flag_size, seq_merge_sort, "seq");
        let par = timed_sort(args.flag_size, merge_sort, "par");
        let speedup = seq as f64 / par as f64;
        println!("speedup: {:.2}x", speedup);
    }
}

#[cfg(test)]
mod bench;
