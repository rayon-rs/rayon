use futures::{self, Future};
use futures::future::lazy;
use ::*;

#[test]
fn future_test() {
    let data = &[0, 1];

    scope(|s| {
        let a = s.spawn_future(futures::future::ok::<_, ()>(&data[0]));
        let b = s.spawn_future(futures::future::ok::<_, ()>(&data[1]));
        let (item1, next) = a.select(b).wait().ok().unwrap();
        let item2 = next.wait().unwrap();
        assert!(*item1 == 0 || *item1 == 1);
        assert!(*item2 == 1 - *item1);
    });
}

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
    future.unwrap();

    assert_eq!(data[0], "Hello, world!");
}

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
    // need 2 threads so we can call `wait()` reliably
    ThreadPool::new(Configuration::new().set_num_threads(2)).unwrap().install(|| {
        scope(|s| {
            let future = s.spawn_future(lazy(move || Ok::<(), ()>(argh())));
            let _ = future.wait(); // should panic, not return a value
        });
    });

    fn argh() -> () {
        if true {
            panic!("Hello, world!");
        }
    }
}
