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

/// Produces fewer items than promised. Does not do any
/// splits at all.
#[test]
#[should_panic(expected = "too few values")]
fn produce_fewer_items() {
    let mut v = vec![];
    let mut collect = Collect::new(&mut v, 5);
    let consumer = collect.as_consumer();
    let mut folder = consumer.into_folder();
    folder = folder.consume(22);
    folder = folder.consume(23);
    folder.complete();
}

// Complete is not called by the consumer.Hence,the collection vector is not fully initialized.
#[test]
#[should_panic(expected = "expected 4 total writes, but got 2")]
fn produce_items_with_split_no_complete() {
    let mut v = vec![];
    let mut collect = Collect::new(&mut v, 4);
    {
        let consumer = collect.as_consumer();
        let (left_consumer, right_consumer, reducer) = consumer.split_at(2);
        let mut left_folder = left_consumer.into_folder();
        let mut right_folder = right_consumer.into_folder();
        left_folder = left_folder.consume(0).consume(1);
        right_folder = right_folder.consume(2).consume(3);
        right_folder.complete();
    }
    collect.complete();
}
