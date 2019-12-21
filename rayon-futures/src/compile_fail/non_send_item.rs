/*! ```compile_fail,E0277
extern crate futures;
extern crate rayon_core;

use std::marker::PhantomData;
use futures::future::poll_fn;
use futures::Poll;
use futures::Async;
use futures::Future;
use rayon_futures::ScopeFutureExt;
use rayon_core::scope;
use rayon_core::ThreadPool;

let evil_future = poll_fn(|| Ok(Async::Ready(PhantomData::<*mut ()>)) );
let pool = ThreadPool::global();

scope(|s| {
    let f = s.spawn_future(evil_future);  //~ ERROR
    let _: Result<_, ()> = f.rayon_wait();
} );
``` */

// Original test case:

/* #[test]
fn non_send_item() {
    use std::marker::PhantomData;
    use std::thread;
    use futures::future::poll_fn;

    struct TattleTale {
        id: thread::ThreadId,
        not_send: PhantomData<*mut ()>
    }

    impl Drop for TattleTale {
        fn drop(&mut self) {
            let drop_id = thread::current().id();
            assert_eq!(self.id, drop_id);
        }
    }

    let evil_future_factory = || { poll_fn(|| {
        let evil_item = TattleTale {
            id: thread::current().id(),
            not_send: PhantomData,
        };
        return Ok(Async::Ready(evil_item));
    } ) };

    let pool = ThreadPool::global();

    scope(|s| {
        let futures: Vec<_> = (0..1000)
            .map(|_: i32| s.spawn_future(evil_future_factory()))
            .collect();

        for f in futures {
            let _: Result<_, ()> = f.rayon_wait();
        }
    } );
} */
