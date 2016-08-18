// Parallel mergesort: regular sorting with a parallel merge step.
// O(n log n) time; O(n) extra space; critical path is O(log^3 n)
//
// Algorithm described in: https://software.intel.com/en-us/articles/a-parallel-stable-sort-using-c11-for-tbb-cilk-plus-and-openmp
extern crate rayon;

use std::cmp::max;
use std::env::args;
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
// TODO: Figure out how to get a and b immutable with enough template magic.
fn rmerge<T: Ord + Send + Copy>(a: &mut [T], b: &mut [T], dest: &mut [T]) {
    // Swap so a is always longer.
    let (a, b) = if a.len() > b.len() {
        (a, b)
    } else {
        (b, a)
    };
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

pub fn is_sorted<T: Send + Ord>(v: &mut [T]) -> bool {
    let n = v.len();
    if n <= SORT_CHUNK {
        for i in 1..n {
            if v[i - 1] >= v[i] {
                return false;
            }
        }
        return true;
    }

    let mid = n / 2;
    if v[mid - 1] >= v[mid] {
        return false;
    }
    let (a, b) = v.split_at_mut(mid);
    let (left, right) = rayon::join(|| is_sorted(a), || is_sorted(b));
    return left && right;
}

fn timed_sort(n: usize) {
    let mut v = Vec::<u32>::with_capacity(n);
    // Populate with unique, pseudorandom values.
    let mut m = 1;
    for _ in 0..n {
        v.push(m);
        m = m.wrapping_mul(101);
    }

    let start = Instant::now();
    merge_sort(&mut v[..]);
    let dur = Instant::now() - start;
    let nanos = dur.subsec_nanos() as u64 + dur.as_secs() * 1_000_000_000u64;
    println!("sorted {} ints: {} s", n, nanos as f32 / 1e9f32);

    // Check correctness
    assert!(is_sorted(&mut v[..]));
}

pub fn main() {
    // Default to gigasort: sort one gigabyte of 32-bit ints.
    let giga: usize = 250_000_000;
    let n: usize = args().nth(1).unwrap_or("".to_string()).parse().unwrap_or(giga);
    timed_sort(n);
}
