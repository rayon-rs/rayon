use futures::{self, Async, Future};
use futures::future::lazy;
use futures::sync::oneshot;
use futures::task::{self, Unpark};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use ::{scope, ThreadPool, Configuration};

/// Basic test of using futures to data on the stack frame.
#[test]
fn future_test() {
    let data = &[0, 1];

    // Here we call `wait` on a select future, which will block at
    // least one thread. So we need a second thread to ensure no
    // deadlock.
    ThreadPool::new(Configuration::new().num_threads(2)).unwrap().install(|| {
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
#[should_panic(expected = "Hello, world!")]
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
    ThreadPool::new(Configuration::new().num_threads(1)).unwrap().install(|| {
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

/// Test that invoking `wait` on a `RayonFuture` will not panic if we
/// are outside a Rayon worker thread.
#[test]
#[should_panic(expected = "Hello, world!")]
fn panicy_unpark() {
    scope(|s| {
        let (a_tx, a_rx) = oneshot::channel::<u32>();
        let rf = s.spawn_future(a_rx);

        // invoke `poll_future` with a `PanicUnpark` instance;
        // this should get installed as a 'waiting task' on the
        // Rayon future `rf`
        let mut spawn = task::spawn(rf);
        let unpark = Arc::new(PanicUnpark);
        match spawn.poll_future(unpark.clone()) {
            Ok(Async::NotReady) => {
                // good, we expect not to be ready yet
            }
            r => panic!("spawn poll returned: {:?}", r),
        }

        // this should trigger the future `a_rx` to be awoken
        // and executing in a Rayon background thread
        a_tx.send(22).unwrap();

        // now we wait for `rf` to complete; when it does, it will
        // also signal the `PanicUnpark` to wake up (that is
        // *supposed* to be what triggers us to `poll` again, but
        // we are sidestepping that)
        let v = spawn.into_inner().rayon_wait().unwrap();
        assert_eq!(v, 22);
    });
    panic!("scope failed to panic!");

    struct PanicUnpark;

    impl Unpark for PanicUnpark {
        fn unpark(&self) {
            panic!("Hello, world!");
        }
    }
}

#[test]
fn double_unpark() {
    let unpark0 = Arc::new(TrackUnpark { value: AtomicUsize::new(0) });
    let unpark1 = Arc::new(TrackUnpark { value: AtomicUsize::new(0) });
    let mut _tag = None;
    scope(|s| {
        let (a_tx, a_rx) = oneshot::channel::<u32>();
        let rf = s.spawn_future(a_rx);

        let mut spawn = task::spawn(rf);

        // test that we don't panic if people try to install a task many times;
        // even if they are different tasks
        for i in 0..22 {
            let u = if i % 2 == 0 {
                unpark0.clone()
            } else {
                unpark1.clone()
            };
            match spawn.poll_future(u) {
                Ok(Async::NotReady) => {
                    // good, we expect not to be ready yet
                }
                r => panic!("spawn poll returned: {:?}", r),
            }
        }

        a_tx.send(22).unwrap();

        // just hold onto `rf` to ensure that nothing is cancelled
        _tag = Some(spawn.into_inner());
    });

    // Since scope is done, our spawned future must have completed. It
    // should have signalled the unpark value we gave it -- but
    // exactly once, even though we called `poll` many times.
    assert_eq!(unpark1.value.load(Ordering::SeqCst), 1);

    // unpark0 was not the last unpark supplied, so it will never be signalled
    assert_eq!(unpark0.value.load(Ordering::SeqCst), 0);

    struct TrackUnpark {
        value: AtomicUsize,
    }

    impl Unpark for TrackUnpark {
        fn unpark(&self) {
            self.value.fetch_add(1, Ordering::SeqCst);
        }
    }
}
