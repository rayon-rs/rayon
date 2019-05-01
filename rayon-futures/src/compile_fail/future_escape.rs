/*! ```compile_fail,E0382,E0501,E0503,E0716

extern crate futures;
extern crate rayon_core;
extern crate rayon_futures;

use futures::future::lazy;
use rayon_futures::ScopeFutureExt;

fn a() {
    let data = &mut [format!("Hello, ")];

    let mut future = None;
    rayon_core::scope(|s| {
        let data = &mut *data;
        future = Some(s.spawn_future(lazy(move || Ok::<_, ()>(&mut data[0]))));
    });

    // `data` is still borrowed as part of future here:
    assert_eq!(data[0], "Hello, world!"); //~ ERROR
}

fn b() {
    let data = &mut [format!("Hello, ")];

    let mut future = None;
    rayon_core::scope(|s| {
        future = Some(s.spawn_future(lazy(move || Ok::<_, ()>(&mut data[0]))));
    });

    // `data` is moved into the scope above, can't use here
    assert_eq!(data[0], "Hello, world!"); //~ ERROR
}

fn c() {
    let mut future = None;
    // borrowed value does not live long enough
    let data = &mut [format!("Hello, ")]; //~ ERROR
    rayon_core::scope(|s| {
        future = Some(s.spawn_future(lazy(move || Ok::<_, ()>(&mut data[0]))));
    });
}

fn main() { }

``` */
