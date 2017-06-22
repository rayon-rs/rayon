use rand::{thread_rng, Rng};
use rayon::prelude::*;
use std::mem;
use test::Bencher;

fn gen_ascending(len: usize) -> Vec<u64> {
    (0..len as u64).collect()
}

fn gen_descending(len: usize) -> Vec<u64> {
    (0..len as u64).rev().collect()
}

fn gen_random(len: usize) -> Vec<u64> {
    let mut rng = thread_rng();
    rng.gen_iter::<u64>().take(len).collect()
}

fn gen_mostly_ascending(len: usize) -> Vec<u64> {
    let mut rng = thread_rng();
    let mut v = gen_ascending(len);
    for _ in (0usize..).take_while(|x| x * x <= len) {
        let x = rng.gen::<usize>() % len;
        let y = rng.gen::<usize>() % len;
        v.swap(x, y);
    }
    v
}

fn gen_mostly_descending(len: usize) -> Vec<u64> {
    let mut rng = thread_rng();
    let mut v = gen_descending(len);
    for _ in (0usize..).take_while(|x| x * x <= len) {
        let x = rng.gen::<usize>() % len;
        let y = rng.gen::<usize>() % len;
        v.swap(x, y);
    }
    v
}

fn gen_strings(len: usize) -> Vec<String> {
    let mut rng = thread_rng();
    let mut v = vec![];
    for _ in 0..len {
        let n = rng.gen::<usize>() % 20 + 1;
        v.push(rng.gen_ascii_chars().take(n).collect());
    }
    v
}

fn gen_big_random(len: usize) -> Vec<[u64; 16]> {
    let mut rng = thread_rng();
    rng.gen_iter().map(|x| [x; 16]).take(len).collect()
}

macro_rules! sort {
    ($f:ident, $name:ident, $gen:expr, $len:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            b.iter(|| $gen($len).$f());
            b.bytes = $len * mem::size_of_val(&$gen(1)[0]) as u64;
        }
    }
}

sort!(par_sort, par_sort_large_ascending, gen_ascending, 50_000);
sort!(par_sort, par_sort_large_descending, gen_descending, 50_000);
sort!(par_sort, par_sort_large_mostly_ascending, gen_mostly_ascending, 50_000);
sort!(par_sort, par_sort_large_mostly_descending, gen_mostly_descending, 50_000);
sort!(par_sort, par_sort_large_random, gen_random, 50_000);
sort!(par_sort, par_sort_large_big_random, gen_big_random, 50_000);
sort!(par_sort, par_sort_large_strings, gen_strings, 50_000);

sort!(par_sort_unstable, par_sort_unstable_large_ascending, gen_ascending, 50_000);
sort!(par_sort_unstable, par_sort_unstable_large_descending, gen_descending, 50_000);
sort!(par_sort_unstable, par_sort_unstable_large_mostly_ascending, gen_mostly_ascending, 50_000);
sort!(par_sort_unstable, par_sort_unstable_large_mostly_descending, gen_mostly_descending, 50_000);
sort!(par_sort_unstable, par_sort_unstable_large_random, gen_random, 50_000);
sort!(par_sort_unstable, par_sort_unstable_large_big_random, gen_big_random, 50_000);
sort!(par_sort_unstable, par_sort_unstable_large_strings, gen_strings, 50_000);
