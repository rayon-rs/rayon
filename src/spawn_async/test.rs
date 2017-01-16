use scope;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;

use {Configuration, PanicHandler, ThreadPool};
use super::spawn_async;

#[test]
fn spawn_then_join_in_worker() {
    let (tx, rx) = channel();
    scope(move |_| {
        spawn_async(move || tx.send(22).unwrap());
    });
    assert_eq!(22, rx.recv().unwrap());
}

#[test]
fn spawn_then_join_outside_worker() {
    let (tx, rx) = channel();
    spawn_async(move || tx.send(22).unwrap());
    assert_eq!(22, rx.recv().unwrap());
}

#[test]
fn panic_fwd() {
    let (tx, rx) = channel();

    let tx = Mutex::new(tx);
    let panic_handler: PanicHandler = Arc::new(move |err| {
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
    });

    let configuration = Configuration::new().set_panic_handler(panic_handler);

    ThreadPool::new(configuration).unwrap().spawn_async(move || panic!("Hello, world!"));

    assert_eq!(1, rx.recv().unwrap());
}
