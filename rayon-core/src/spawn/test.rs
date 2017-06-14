#[cfg(rayon_unstable)]
use futures::{lazy, Future};

use scope;
use std::any::Any;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;

use {Configuration, ThreadPool};
use super::spawn;
#[cfg(rayon_unstable)]
use super::spawn_future;

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

    let configuration = Configuration::new().panic_handler(panic_handler);

    ThreadPool::new(configuration).unwrap().spawn(move || panic!("Hello, world!"));

    assert_eq!(1, rx.recv().unwrap());
}

#[test]
#[cfg(rayon_unstable)]
fn async_future_map() {
    let data = Arc::new(Mutex::new(format!("Hello, ")));

    let a = spawn_future(lazy({
        let data = data.clone();
        move || Ok::<_, ()>(data)
    }));
    let future = spawn_future(a.map(|data| {
        let mut v = data.lock().unwrap();
        v.push_str("world!");
    }));
    let () = future.wait().unwrap();

    // future must have executed for the scope to have ended, even
    // though we never invoked `wait` to observe its result
    assert_eq!(&data.lock().unwrap()[..], "Hello, world!");
}

#[test]
#[should_panic(expected = "Hello, world!")]
#[cfg(rayon_unstable)]
fn async_future_panic_prop() {
    let future = spawn_future(lazy(move || Ok::<(), ()>(argh())));
    let _ = future.rayon_wait(); // should panic, not return a value

    fn argh() -> () {
        if true {
            panic!("Hello, world!");
        }
    }
}

#[test]
#[cfg(rayon_unstable)]
fn async_future_scope_interact() {
    let future = spawn_future(lazy(move || Ok::<usize, ()>(22)));

    let mut vec = vec![];
    scope(|s| {
        let future = s.spawn_future(future.map(|x| x * 2));
        s.spawn(|_| {
            vec.push(future.rayon_wait().unwrap());
        }); // just because
    });

    assert_eq!(vec![44], vec);
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
        let thread_pool = ThreadPool::new(Configuration::new()).unwrap();
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
    let config = Configuration::new().panic_handler(panic_handler);
    ThreadPool::new(config).unwrap().spawn(move || {
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
    let config = Configuration::new().panic_handler(panic_handler);
    ThreadPool::new(config).unwrap().spawn(move || {
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
