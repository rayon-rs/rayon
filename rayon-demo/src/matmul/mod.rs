const USAGE: &'static str = "
Usage: matmul bench [--size N]
       matmul --help
Parallel matrix multiplication.

Commands:
    bench           Run the benchmark in different modes and print the timings.
Options:
    --size N        Row-size of matrices (rounded up to power of 2) [default: 1024]
    -h, --help      Show this message.
";

#[derive(RustcDecodable)]
pub struct Args {
    cmd_bench: bool,
    flag_size: usize,
}

use docopt::Docopt;
use rayon;
use rayon::prelude::*;

use std::time::Instant;

// TODO: Investigate other cache patterns for row-major order that may be more
// parallelizable.
// https://tavianator.com/a-quick-trick-for-faster-naive-matrix-multiplication/
pub fn seq_matmul(a: &[f32], b: &[f32], dest: &mut [f32]) {
    // Zero dest, as it may be uninitialized.
    for d in dest.iter_mut() {
        *d = 0.0;
    }
    // Multiply in row-major order.
    // D[i,j] = sum for all k A[i,k] * B[k,j]
    let bits = dest.len().trailing_zeros() / 2;
    let n = 1 << bits;
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += unsafe { a.get_unchecked(i << bits | k) * b.get_unchecked(k << bits | j) };
            }
            dest[i << bits | j] = sum;
        }
    }
}

// Iterator that counts in interleaved bits.
// e.g. 0b0, 0b1, 0b100, 0b101, 0b10000, 0b10001, ...
struct SplayedBitsCounter {
    value: usize,
    max: usize,
}

impl SplayedBitsCounter {
    fn new(max: usize) -> Self {
        SplayedBitsCounter { value: 0, max: max }
    }
}

impl Iterator for SplayedBitsCounter {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        // Return only odd bits.
        let prev = self.value & 0x55555555;
        if prev < self.max {
            // Set all even bits.
            self.value |= 0xaaaaaaaa;
            // Add one, carrying through even bits.
            self.value += 1;
            Some(prev)
        } else {
            None
        }
    }
}

#[test]
fn test_splayed_counter() {
    let bits: Vec<usize> = SplayedBitsCounter::new(64).collect();
    assert_eq!(
        vec![0b0, 0b1, 0b100, 0b101, 0b10000, 0b10001, 0b10100, 0b10101],
        bits
    );
}

// Multiply the matrices laid out in z order.
// https://en.wikipedia.org/wiki/Z-order_curve
#[inline(never)]
pub fn seq_matmulz(a: &[f32], b: &[f32], dest: &mut [f32]) {
    // All inputs need to be the same length.
    assert!(a.len() == b.len() && a.len() == dest.len());
    // Input matrices must be square with each side a power of 2.
    assert!(a.len().count_ones() == 1 && a.len().trailing_zeros() % 2 == 0);
    // Zero dest, as it may be uninitialized.
    for d in dest.iter_mut() {
        *d = 0.0;
    }

    // Multiply in morton order
    // D[i,j] = sum for all k A[i,k] * B[k,j]
    let n = dest.len();
    for ij in 0..n {
        let i = ij & 0xaaaaaaaa;
        let j = ij & 0x55555555;
        let mut sum = 0.0;
        for k in SplayedBitsCounter::new(n) {
            // sum += a[i, k] * b[k, j];
            sum += unsafe {
                // If n is a power of 4: i, j, or k should produce
                // no bits outside a valid index of n.
                a.get_unchecked(i | k) * b.get_unchecked(k << 1 | j)
            };
        }
        dest[ij] = sum;
    }
}

const MULT_CHUNK: usize = 1 * 1024;
const LINEAR_CHUNK: usize = 64 * 1024;

fn quarter_chunks<'a>(v: &'a [f32]) -> (&'a [f32], &'a [f32], &'a [f32], &'a [f32]) {
    let mid = v.len() / 2;
    let quarter = mid / 2;
    let (left, right) = v.split_at(mid);
    let (a, b) = left.split_at(quarter);
    let (c, d) = right.split_at(quarter);
    (a, b, c, d)
}

fn quarter_chunks_mut<'a>(v: &'a mut [f32],)
    -> (&'a mut [f32], &'a mut [f32], &'a mut [f32], &'a mut [f32]) {
    let mid = v.len() / 2;
    let quarter = mid / 2;
    let (left, right) = v.split_at_mut(mid);
    let (a, b) = left.split_at_mut(quarter);
    let (c, d) = right.split_at_mut(quarter);
    (a, b, c, d)
}

fn join4<F1, F2, F3, F4, R1, R2, R3, R4>(f1: F1, f2: F2, f3: F3, f4: F4) -> (R1, R2, R3, R4)
where
    F1: FnOnce() -> R1 + Send,
    R1: Send,
    F2: FnOnce() -> R2 + Send,
    R2: Send,
    F3: FnOnce() -> R3 + Send,
    R3: Send,
    F4: FnOnce() -> R4 + Send,
    R4: Send,
{
    let ((r1, r2), (r3, r4)) = rayon::join(|| rayon::join(f1, f2), || rayon::join(f3, f4));
    (r1, r2, r3, r4)
}

fn join8<F1, F2, F3, F4, F5, F6, F7, F8, R1, R2, R3, R4, R5, R6, R7, R8>
    (
    f1: F1,
    f2: F2,
    f3: F3,
    f4: F4,
    f5: F5,
    f6: F6,
    f7: F7,
    f8: F8,
) -> (R1, R2, R3, R4, R5, R6, R7, R8)
where
    F1: FnOnce() -> R1 + Send,
    R1: Send,
    F2: FnOnce() -> R2 + Send,
    R2: Send,
    F3: FnOnce() -> R3 + Send,
    R3: Send,
    F4: FnOnce() -> R4 + Send,
    R4: Send,
    F5: FnOnce() -> R5 + Send,
    R5: Send,
    F6: FnOnce() -> R6 + Send,
    R6: Send,
    F7: FnOnce() -> R7 + Send,
    R7: Send,
    F8: FnOnce() -> R8 + Send,
    R8: Send,
{
    let (((r1, r2), (r3, r4)), ((r5, r6), (r7, r8))) =
        rayon::join(
            || rayon::join(|| rayon::join(f1, f2), || rayon::join(f3, f4)),
            || rayon::join(|| rayon::join(f5, f6), || rayon::join(f7, f8)),
        );
    (r1, r2, r3, r4, r5, r6, r7, r8)
}

// Multiply two square power of two matrices, given in Z-order.
pub fn matmulz(a: &[f32], b: &[f32], dest: &mut [f32]) {
    if a.len() <= MULT_CHUNK {
        seq_matmulz(a, b, dest);
        return;
    }

    // Allocate uninitialized scratch space.
    let mut tmp = raw_buffer(dest.len());

    let (a1, a2, a3, a4) = quarter_chunks(a);
    let (b1, b2, b3, b4) = quarter_chunks(b);
    {
        let (d1, d2, d3, d4) = quarter_chunks_mut(dest);
        let (t1, t2, t3, t4) = quarter_chunks_mut(&mut tmp[..]);
        // Multiply 8 submatrices
        join8(
            || matmulz(a1, b1, d1),
            || matmulz(a1, b2, d2),
            || matmulz(a3, b1, d3),
            || matmulz(a3, b2, d4),
            || matmulz(a2, b3, t1),
            || matmulz(a2, b4, t2),
            || matmulz(a4, b3, t3),
            || matmulz(a4, b4, t4),
        );
    }

    // Sum each quarter
    rmatsum(tmp.as_mut(), dest);
}

pub fn matmul_strassen(a: &[f32], b: &[f32], dest: &mut [f32]) {
    if a.len() <= MULT_CHUNK {
        seq_matmulz(a, b, dest);
        return;
    }

    // Naming taken from https://en.wikipedia.org/wiki/Strassen_algorithm
    let (a11, a12, a21, a22) = quarter_chunks(a);
    let (b11, b12, b21, b22) = quarter_chunks(b);
    // 7 submatrix multiplies.
    // Maybe the tree should be leaning the other way...
    let (m1, m2, m3, m4, m5, m6, m7, _) = join8(
        || strassen_add2_mul(a11, a22, b11, b22),
        || strassen_add_mul(a21, a22, b11),
        || strassen_sub_mul(b12, b22, a11),
        || strassen_sub_mul(b21, b11, a22),
        || strassen_add_mul(a11, a12, b22),
        || strassen_sub_add_mul(a21, a11, b11, b12),
        || strassen_sub_add_mul(a12, a22, b21, b22),
        || (),
    );

    // Sum results into dest.
    let (c11, c12, c21, c22) = quarter_chunks_mut(dest);
    join4(
        || strassen_sum_sub(&m1[..], &m4[..], &m7[..], &m5[..], c11),
        || strassen_sum(&m3[..], &m5[..], c12),
        || strassen_sum(&m2[..], &m4[..], c21),
        || strassen_sum_sub(&m1[..], &m3[..], &m6[..], &m2[..], c22),
    );
}

fn raw_buffer(n: usize) -> Vec<f32> {
    let mut tmp = Vec::with_capacity(n);
    unsafe {
        tmp.set_len(n);
    }
    tmp
}

fn strassen_add2_mul(a1: &[f32], a2: &[f32], b1: &[f32], b2: &[f32]) -> Vec<f32> {
    let mut dest = raw_buffer(a1.len());
    let (a, b) = rayon::join(|| rtmp_sum(a1, a2), || rtmp_sum(b1, b2));
    matmul_strassen(&a[..], &b[..], &mut dest[..]);
    dest
}

fn strassen_sub_add_mul(a1: &[f32], a2: &[f32], b1: &[f32], b2: &[f32]) -> Vec<f32> {
    let mut dest = raw_buffer(a1.len());
    let (a, b) = rayon::join(|| rtmp_sub(a1, a2), || rtmp_sum(b1, b2));
    matmul_strassen(&a[..], &b[..], &mut dest[..]);
    dest
}

fn strassen_add_mul(a1: &[f32], a2: &[f32], b: &[f32]) -> Vec<f32> {
    let mut dest = raw_buffer(a1.len());
    let a = rtmp_sum(a1, a2);
    matmul_strassen(&a[..], b, &mut dest[..]);
    dest
}

fn strassen_sub_mul(b1: &[f32], b2: &[f32], a: &[f32]) -> Vec<f32> {
    let mut dest = raw_buffer(a.len());
    let b = rtmp_sub(b1, b2);
    matmul_strassen(a, &b[..], &mut dest[..]);
    dest
}

fn strassen_sum_sub(a: &[f32], b: &[f32], c: &[f32], s: &[f32], dest: &mut [f32]) {
    rcopy(a, dest);
    rmatsum(b, dest);
    rmatsum(c, dest);
    rmatsub(s, dest);
}

fn strassen_sum(a: &[f32], b: &[f32], dest: &mut [f32]) {
    rcopy(a, dest);
    rmatsum(b, dest);
}

fn rtmp_sum(a: &[f32], b: &[f32]) -> Vec<f32> {
    let mut tmp = raw_buffer(a.len());
    rcopy(a, &mut tmp[..]);
    rmatsum(b, &mut tmp[..]);
    tmp
}

fn rtmp_sub(a: &[f32], b: &[f32]) -> Vec<f32> {
    let mut tmp = raw_buffer(a.len());
    rcopy(a, &mut tmp[..]);
    rmatsub(b, &mut tmp[..]);
    tmp
}

// Any layout works, we're just adding by element.
fn rmatsum(src: &[f32], dest: &mut [f32]) {
    dest.par_iter_mut()
        .zip(src.par_iter())
        .for_each(|(d, s)| *d += *s);
}

fn rmatsub(src: &[f32], dest: &mut [f32]) {
    dest.par_iter_mut()
        .zip(src.par_iter())
        .for_each(|(d, s)| *d -= *s);
}

fn rcopy(src: &[f32], dest: &mut [f32]) {
    if dest.len() <= LINEAR_CHUNK {
        dest.copy_from_slice(src);
        return;
    }

    let mid = dest.len() / 2;
    let (s1, s2) = src.split_at(mid);
    let (d1, d2) = dest.split_at_mut(mid);
    rayon::join(|| rcopy(s1, d1), || rcopy(s2, d2));
}

#[test]
fn test_matmul() {
    // Verify that small matrix gets the right result.
    let a: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    let b: Vec<f32> = vec![5.0, 6.0, 7.0, 8.0];
    let mul: Vec<f32> = vec![19.0, 22.0, 43.0, 50.0];
    let mut dest = vec![0f32; 4];
    matmulz(&a[..], &b[..], &mut dest[..]);
    assert_eq!(mul, dest);

    seq_matmulz(&a[..], &b[..], &mut dest[..]);
    assert_eq!(mul, dest);

    matmul_strassen(&a[..], &b[..], &mut dest[..]);
    assert_eq!(mul, dest);

    // Verify that large matrix gets the same results in parallel and serial algorithms.
    let n = 1 << 14;
    assert!(n > MULT_CHUNK); // If we don't recurse we're not testing much.
    let a: Vec<f32> = (0..n).map(|i| (i % 101) as f32).collect();
    let b: Vec<f32> = (0..n).map(|i| (i % 101 + 7) as f32).collect();
    let mut seqmul = vec![0f32; n];
    seq_matmulz(&a[..], &b[..], &mut seqmul[..]);
    let mut rmul = vec![0f32; n];
    matmulz(&a[..], &b[..], &mut rmul[..]);
    assert_eq!(rmul, seqmul);

    // Verify strassen gets the same results.
    for d in rmul.iter_mut() {
        *d = 0.0;
    }
    matmul_strassen(&a[..], &b[..], &mut rmul[..]);
    assert_eq!(rmul, seqmul);
}

fn timed_matmul<F: FnOnce(&[f32], &[f32], &mut [f32])>(size: usize, f: F, name: &str) -> u64 {
    let size = size.next_power_of_two();
    let n = size * size;
    let mut a = vec![0f32; n];
    let mut b = vec![0f32; n];
    for i in 0..n {
        a[i] = i as f32;
        b[i] = (i + 7) as f32;
    }
    let mut dest = vec![0f32; n];

    let start = Instant::now();
    f(&a[..], &b[..], &mut dest[..]);
    let dur = Instant::now() - start;
    let nanos = dur.subsec_nanos() as u64 + dur.as_secs() * 1_000_000_000u64;
    println!(
        "{}:\t{}x{} matrix: {} s",
        name,
        size,
        size,
        nanos as f32 / 1e9f32
    );
    return nanos;
}

pub fn main(args: &[String]) {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(args).decode())
        .unwrap_or_else(|e| e.exit());

    if args.cmd_bench {
        if args.flag_size <= 1024 {
            // Crappy algorithm takes several minutes on larger inputs.
            timed_matmul(args.flag_size, seq_matmul, "seq row-major");
        }
        let seq = if args.flag_size <= 2048 {
            timed_matmul(args.flag_size, seq_matmulz, "seq z-order")
        } else {
            0
        };
        let par = timed_matmul(args.flag_size, matmulz, "par z-order");
        timed_matmul(args.flag_size, matmul_strassen, "par strassen");
        let speedup = seq as f64 / par as f64;
        println!("speedup: {:.2}x", speedup);
    }
}

#[cfg(test)]
mod bench;
