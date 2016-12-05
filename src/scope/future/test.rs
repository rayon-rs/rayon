use futures::{self, Future};
use futures::future::lazy;
use ::*;

/// Basic test of using futures to data on the stack frame.
#[test]
fn future_test() {
    let data = &[0, 1];

    // Here we call `wait` on a select future, which will block at
    // least one thread. So we need a second thread to ensure no
    // deadlock.
    ThreadPool::new(Configuration::new().set_num_threads(2)).unwrap().install(|| {
        scope(|s| {
            let a = s.spawn_future(futures::future::ok::<_, ()>(&data[0]));
            let b = s.spawn_future(futures::future::ok::<_, ()>(&data[1]));
            let (item1, next) = a.select(b).wait().ok().unwrap();
            let item2 = next.wait().unwrap();
            assert!(*item1 == 0 || *item1 == 1);
            assert!(*item2 == 1 - *item1);
        });
    });
}

/// Test using `map` on a Rayon future. The `map` closure is eecuted
/// for side-effects, and modifies the `data` variable that is owned
/// by enclosing stack frame.
#[test]
fn future_map() {
    let data = &mut [format!("Hello, ")];

    let mut future = None;
    scope(|s| {
        let a = s.spawn_future(lazy(|| Ok::<_, ()>(&mut data[0])));
        future = Some(s.spawn_future(a.map(|v| {
            v.push_str("world!");
        })));
    });

    // future must have executed for the scope to have ended, even
    // though we never invoked `wait` to observe its result
    assert_eq!(data[0], "Hello, world!");
    assert!(future.is_some());
}

/// Test that we can create a future that returns an `&mut` to data,
/// so long as it outlives the scope.
#[test]
fn future_escape_ref() {
    let data = &mut [format!("Hello, ")];

    {
        let mut future = None;
        scope(|s| {
            let data = &mut *data;
            future = Some(s.spawn_future(lazy(move || Ok::<_, ()>(&mut data[0]))));
        });
        let s = future.unwrap().wait().unwrap();
        s.push_str("world!");
    }

    assert_eq!(data[0], "Hello, world!");
}

#[test]
#[should_panic]
fn future_panic_prop() {
    scope(|s| {
        let future = s.spawn_future(lazy(move || Ok::<(), ()>(argh())));
        let _ = future.rayon_wait(); // should panic, not return a value
    });

    fn argh() -> () {
        if true {
            panic!("Hello, world!");
        }
    }
}

/// Test that, even if we have only one thread, invoke `rayon_wait`
/// will not panic.
#[test]
fn future_rayon_wait_1_thread() {
    // run with only 1 worker thread; this would deadlock if we couldn't make progress
    let mut result = None;
    ThreadPool::new(Configuration::new().set_num_threads(1)).unwrap().install(|| {
        scope(|s| {
            use std::sync::mpsc::channel;
            let (tx, rx) = channel();
            let a = s.spawn_future(lazy(move || Ok::<usize, ()>(rx.recv().unwrap())));
            //                          ^^^^ FIXME: why is this needed?
            let b = s.spawn_future(a.map(|v| v + 1));
            let c = s.spawn_future(b.map(|v| v + 1));
            s.spawn(move |_| tx.send(20).unwrap());
            result = Some(c.rayon_wait().unwrap());
        });
    });
    assert_eq!(result, Some(22));
}

/// Test that invoking `wait` on a `RayonFuture` will panic, if it is inside
/// a Rayon worker thread.
#[test]
#[should_panic]
fn future_wait_panics_inside_rayon_thread() {
    scope(|s| {
        let future = s.spawn_future(lazy(move || Ok::<(), ()>(())));
        let _ = future.wait(); // should panic, not return a value
    });
}


/// Test that invoking `wait` on a `RayonFuture` will not panic if we
/// are outside a Rayon worker thread.
#[test]
fn future_wait_works_outside_rayon_threads() {
    let mut future = None;
    scope(|s| {
        future = Some(s.spawn_future(lazy(move || Ok::<(), ()>(()))));
    });
    assert_eq!(Ok(()), future.unwrap().wait());
}

