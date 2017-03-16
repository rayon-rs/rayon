#![cfg(test)]

// These tests are primarily targeting "abusive" producers that will
// try to drive the "collect consumer" incorrectly. These should
// result in panics.

use iter::internal::*;
use super::Collect;

/// Promises to produce 2 items, but then produces 3.  Does not do any
/// splits at all.
#[test]
#[should_panic(expected = "too many values")]
fn produce_too_many_items() {
    let mut v = vec![];
    let mut collect = Collect::new(&mut v, 2);
    let consumer = collect.as_consumer();
    let mut folder = consumer.into_folder();
    folder = folder.consume(22);
    folder = folder.consume(23);
    folder.consume(24);
}
