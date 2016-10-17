use {scope, Scope};
use prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn scope_empty() {
    scope(|_| { });
}

#[test]
fn scope_two() {
    let counter = &AtomicUsize::new(0);
    scope(|s| {
        s.spawn(move |_| { counter.fetch_add(1, Ordering::SeqCst); });
        s.spawn(move |_| { counter.fetch_add(10, Ordering::SeqCst); });
    });

    let v = counter.load(Ordering::SeqCst);
    assert_eq!(v, 11);
}

#[test]
fn scope_divide_and_conquer() {
    let counter_p = &AtomicUsize::new(0);
    scope(|s| s.spawn(move |s| divide_and_conquer(s, counter_p, 1024)));

    let counter_s = &AtomicUsize::new(0);
    divide_and_conquer_seq(&counter_s, 1024);

    let p = counter_p.load(Ordering::SeqCst);
    let s = counter_s.load(Ordering::SeqCst);
    assert_eq!(p, s);
}

fn divide_and_conquer<'scope>(
    scope: &Scope<'scope>, counter: &'scope AtomicUsize, size: usize)
{
    if size > 1 {
        scope.spawn(move |scope| divide_and_conquer(scope, counter, size / 2));
        scope.spawn(move |scope| divide_and_conquer(scope, counter, size / 2));
    } else {
        // count the leaves
        counter.fetch_add(1, Ordering::SeqCst);
    }
}

fn divide_and_conquer_seq(counter: &AtomicUsize, size: usize) {
    if size > 1 {
        divide_and_conquer_seq(counter, size / 2);
        divide_and_conquer_seq(counter, size / 2);
    } else {
        // count the leaves
        counter.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn scope_mix() {
    let counter_p = &AtomicUsize::new(0);
    scope(|s| {
        s.spawn(move |s| {
            divide_and_conquer(s, counter_p, 1024);
        });
        s.spawn(move |_| {
            let a: Vec<i32> = (0..1024).collect();
            let r1 = a.par_iter()
                      .weight_max()
                      .map(|&i| i + 1)
                      .reduce_with(|i, j| i + j);
            let r2 = a.iter()
                      .map(|&i| i + 1)
                      .fold(0, |a,b| a+b);
            assert_eq!(r1.unwrap(), r2);
        });
    });
}
