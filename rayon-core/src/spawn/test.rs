use scope;
use std::any::Any;
use std::sync::Mutex;
use std::sync::mpsc::channel;

use ThreadPoolBuilder;
use super::spawn;

#[test]
fn spawn_then_join_in_worker() {
    let (tx, rx) = channel();
    scope(move |_| {
        spawn(move || tx.send(22).unwrap());
    });
    assert_eq!(22, rx.recv().unwrap());
}

#[test]
fn spawn_then_join_outside_worker() {
    let (tx, rx) = channel();
    spawn(move || tx.send(22).unwrap());
    assert_eq!(22, rx.recv().unwrap());
}

#[test]
fn panic_fwd() {
    let (tx, rx) = channel();

    let tx = Mutex::new(tx);
    let panic_handler = move |err: Box<Any + Send>| {
        let tx = tx.lock().unwrap();
        if let Some(&msg) = err.downcast_ref::<&str>() {
            if msg == "Hello, world!" {
                tx.send(1).unwrap();
            } else {
                tx.send(2).unwrap();
            }
        } else {
            tx.send(3).unwrap();
        }
    };

    let builder = ThreadPoolBuilder::new().panic_handler(panic_handler);

    builder.build().unwrap().spawn(move || panic!("Hello, world!"));

    assert_eq!(1, rx.recv().unwrap());
}

/// Test what happens when the thread-pool is dropped but there are
/// still active asynchronous tasks. We expect the thread-pool to stay
/// alive and executing until those threads are complete.
#[test]
fn termination_while_things_are_executing() {
    let (tx0, rx0) = channel();
    let (tx1, rx1) = channel();

    // Create a thread-pool and spawn some code in it, but then drop
    // our reference to it.
    {
        let thread_pool = ThreadPoolBuilder::new().build().unwrap();
        thread_pool.spawn(move || {
            let data = rx0.recv().unwrap();

            // At this point, we know the "main" reference to the
            // `ThreadPool` has been dropped, but there are still
            // active threads. Launch one more.
            spawn(move || {
                tx1.send(data).unwrap();
            });
        });
    }

    tx0.send(22).unwrap();
    let v = rx1.recv().unwrap();
    assert_eq!(v, 22);
}

#[test]
fn custom_panic_handler_and_spawn() {
    let (tx, rx) = channel();

    // Create a parallel closure that will send panics on the
    // channel; since the closure is potentially executed in parallel
    // with itself, we have to wrap `tx` in a mutex.
    let tx = Mutex::new(tx);
    let panic_handler = move |e: Box<Any + Send>| {
        tx.lock().unwrap().send(e).unwrap();
    };

    // Execute an async that will panic.
    let builder = ThreadPoolBuilder::new().panic_handler(panic_handler);
    builder.build().unwrap().spawn(move || {
        panic!("Hello, world!");
    });

    // Check that we got back the panic we expected.
    let error = rx.recv().unwrap();
    if let Some(&msg) = error.downcast_ref::<&str>() {
        assert_eq!(msg, "Hello, world!");
    } else {
        panic!("did not receive a string from panic handler");
    }
}

#[test]
fn custom_panic_handler_and_nested_spawn() {
    let (tx, rx) = channel();

    // Create a parallel closure that will send panics on the
    // channel; since the closure is potentially executed in parallel
    // with itself, we have to wrap `tx` in a mutex.
    let tx = Mutex::new(tx);
    let panic_handler = move |e| {
        tx.lock().unwrap().send(e).unwrap();
    };

    // Execute an async that will (eventually) panic.
    const PANICS: usize = 3;
    let builder = ThreadPoolBuilder::new().panic_handler(panic_handler);
    builder.build().unwrap().spawn(move || {
        // launch 3 nested spawn-asyncs; these should be in the same
        // thread-pool and hence inherit the same panic handler
        for _ in 0 .. PANICS {
            spawn(move || {
                panic!("Hello, world!");
            });
        }
    });

    // Check that we get back the panics we expected.
    for _ in 0 .. PANICS {
        let error = rx.recv().unwrap();
        if let Some(&msg) = error.downcast_ref::<&str>() {
            assert_eq!(msg, "Hello, world!");
        } else {
            panic!("did not receive a string from panic handler");
        }
    }
}
