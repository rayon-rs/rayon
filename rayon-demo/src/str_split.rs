//! Some microbenchmarks for splitting strings

use rand::seq::SliceRandom;
use rayon::prelude::*;
use std::sync::LazyLock;
use test::Bencher;

static HAYSTACK: LazyLock<String> = LazyLock::new(|| {
    let mut rng = crate::seeded_rng();
    let mut bytes: Vec<u8> = "abcdefg ".bytes().cycle().take(1_000_000).collect();
    bytes.shuffle(&mut rng);
    String::from_utf8(bytes).unwrap()
});

static COUNT: LazyLock<usize> = LazyLock::new(|| HAYSTACK.split(' ').count());

// Try multiple kinds of whitespace, but HAYSTACK only contains plain spaces.
const WHITESPACE: &[char] = &['\r', '\n', ' ', '\t'];

fn get_string_count() -> (&'static str, usize) {
    (&HAYSTACK, *COUNT)
}

#[bench]
fn parallel_space_char(b: &mut Bencher) {
    let (string, count) = get_string_count();
    b.iter(|| assert_eq!(string.par_split(' ').count(), count))
}

#[bench]
fn parallel_space_chars(b: &mut Bencher) {
    let (string, count) = get_string_count();
    b.iter(|| assert_eq!(string.par_split(WHITESPACE).count(), count))
}

#[bench]
fn parallel_space_fn(b: &mut Bencher) {
    let (string, count) = get_string_count();
    b.iter(|| assert_eq!(string.par_split(|c| c == ' ').count(), count))
}

#[bench]
fn serial_space_char(b: &mut Bencher) {
    let (string, count) = get_string_count();
    b.iter(|| assert_eq!(string.split(' ').count(), count))
}

#[bench]
fn serial_space_chars(b: &mut Bencher) {
    let (string, count) = get_string_count();
    b.iter(|| assert_eq!(string.split(WHITESPACE).count(), count))
}

#[bench]
#[expect(clippy::manual_pattern_char_comparison)]
fn serial_space_fn(b: &mut Bencher) {
    let (string, count) = get_string_count();
    b.iter(|| assert_eq!(string.split(|c| c == ' ').count(), count))
}

#[bench]
fn serial_space_str(b: &mut Bencher) {
    let (string, count) = get_string_count();
    #[allow(clippy::single_char_pattern)]
    b.iter(|| assert_eq!(string.split(" ").count(), count))
}
