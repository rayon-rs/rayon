extern crate futures;
extern crate rayon;

use futures::future::lazy;
use rayon::scope;

fn a() {
    let data = &mut [format!("Hello, ")];

    let mut future = None;
    scope(|s| {
        let data = &mut *data;
        future = Some(s.spawn_future(lazy(move || Ok::<_, ()>(&mut data[0]))));
    });

    // `data` is still borrowed as part of future here:
    assert_eq!(data[0], "Hello, world!"); //~ ERROR E0501
}

fn b() {
    let data = &mut [format!("Hello, ")];

    let mut future = None;
    scope(|s| {
        future = Some(s.spawn_future(lazy(move || Ok::<_, ()>(&mut data[0]))));
    });

    // `data` is moved into the scope above, can't use here
    assert_eq!(data[0], "Hello, world!"); //~ ERROR E0382
}

fn c() {
    let mut future = None;
    let data = &mut [format!("Hello, ")];
    scope(|s| {
        future = Some(s.spawn_future(lazy(move || Ok::<_, ()>(&mut data[0]))));
    });
} //~ ERROR borrowed value does not live long enough

fn main() { }
