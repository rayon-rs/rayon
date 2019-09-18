/*! ```
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
    let f = s.spawn_future(evil_future);
    let _: Result<_, ()> = f.rayon_waist();
} );
*/
