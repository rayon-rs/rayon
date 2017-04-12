#![cfg(feature = "unstable")]

use futures::lazy;
use prelude::*;
use iter::internal::*;
use std::collections::VecDeque;
use Scope;

mod test;

pub trait IntoParPipeline
    where Self: Sized + IntoIterator, Self::Item: Send,
{
    /// Converts a sequential iterator into a parallel one. The
    /// sequential iterator will be driven from a single thread, and a
    /// Rayon task will be created for each item in the iterator.
    ///
    /// In general, it is preferable to use a native parallel iterator
    /// if they are available. It may also be better to collect the
    /// items into a vector and then use `into_par_iter()` on that
    /// vector -- this can help us to create fewer parallel tasks and
    /// reduce overall overhead.
    ///
    /// If the sequential iterator is not `Send`, see `into_par_pipeline_scoped()`.
    fn into_par_pipeline(self) -> Pipeline<Self> where Self: Send;

    /// Like `into_par_pipeline()`, but also works for iterators that
    /// are not `Send`. Requires that your code is within a Rayon
    /// scope, which ensures that the code is already running on the
    /// Rayon threadpool.
    ///
    /// Example:
    ///
    /// ```
    /// use rayon::prelude::*;
    /// use std::rc::Rc; // `Rc` is not `Send`
    /// rayon::scope(|s| {
    ///     let v = vec![Rc::new(1), Rc::new(2)];
    ///     v.into_iter()
    ///      .map(|r| *r) // iterating over i32s
    ///      .into_par_pipeline_scoped(s)
    ///      .find_any(|&n| n == 1);
    /// });
    /// ```
    fn into_par_pipeline_scoped<'s, 'scope>(self, scope: &'s Scope<'scope>) -> PipelineScoped<'s, 'scope, Self>;
}

impl<I> IntoParPipeline for I
    where I: IntoIterator, I::Item: Send,
{
    fn into_par_pipeline(self) -> Pipeline<Self> where Self: Send {
        Pipeline { iter: self, depth: 0 }
    }

    fn into_par_pipeline_scoped<'s, 'scope>(self, scope: &'s Scope<'scope>) -> PipelineScoped<'s, 'scope, Self> {
        PipelineScoped { iter: self, depth: 0, _scope: scope }
    }
}

pub struct Pipeline<I>
    where I: IntoIterator + Send, I::Item: Send,
{
    iter: I,
    depth: usize,
}

impl<I> Pipeline<I>
    where I: IntoIterator + Send, I::Item: Send,
{
    /// The "depth" is a tuning parameter that affects how much time
    /// we will spend drawing things out of the iterator versus
    /// attempting to process them. Using a higher number will cause
    /// us to pull more items out of the iterator initially. Using 0
    /// signals that you wish us to use the default behavior and
    /// attempt to find a suitable depth.
    pub fn with_depth(self, depth: usize) -> Self {
        Pipeline { depth: depth, ..self }
    }
}

impl<I> ParallelIterator for Pipeline<I>
    where I: IntoIterator + Send, I::Item: Send,
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result where C: UnindexedConsumer<Self::Item> {
        let iter = self.iter;
        let depth = self.depth;
        ::scope(|scope| drive(scope, depth, iter, consumer))
    }
}

pub struct PipelineScoped<'s, 'scope, I>
    where I: IntoIterator, I::Item: Send, 'scope: 's,
{
    iter: I,

    depth: usize,

    // This field exists just to prove that we are already in a Rayon
    // worker thread, really.
    _scope: &'s Scope<'scope>,
}

impl<'s, 'scope, I> PipelineScoped<'s, 'scope, I>
    where I: IntoIterator, I::Item: Send, 'scope: 's,
{
    /// The "depth" is a tuning parameter that affects how much time
    /// we will spend drawing things out of the iterator versus
    /// attempting to process them. Using a higher number will cause
    /// us to pull more items out of the iterator initially. Using 0
    /// signals that you wish us to use the default behavior and
    /// attempt to find a suitable depth.
    pub fn with_depth(self, depth: usize) -> Self {
        PipelineScoped { depth: depth, ..self }
    }
}

impl<'s, 'scope, I> ParallelIterator for PipelineScoped<'s, 'scope, I>
    where I: IntoIterator, I::Item: Send,
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result where C: UnindexedConsumer<Self::Item> {
        // The `Sneaky` type lets you move a value across threads that
        // is not `Send`. In this case, it is ok for us to use it,
        // because we know that we are currently in a worker thread
        // (since we have access to `self.scope`), and hence
        // `rayon::scope` will actually execute its closure in the
        // same thread we started from (so we are not *really* moving
        // things across threads).
        struct Sneaky<T> { iter: T }
        unsafe impl<T> Send for Sneaky<T> { }

        // This thread-local exists *just* to assert the above property.
        use std::cell::Cell;
        thread_local! {
            static PTR: Cell<usize> = Cell::new(0);
        }

        let my_stack: usize = 22;
        let my_stack_p: usize = &my_stack as *const usize as usize;
        PTR.with(|p| p.set(my_stack_p));

        let depth = self.depth;
        let sneaky = Sneaky { iter: self.iter };
        ::scope(move |scope| {
            // assert that we are still on the same thread we started
            // from; sure would be nice to have a thread-id
            PTR.with(|p| {
                let q = p.get();
                p.set(0);
                assert!(q == my_stack_p);
            });

            let Sneaky { iter } = sneaky;
            drive(scope, depth, iter, consumer)
        })
    }
}

fn drive<'scope, I, C>(scope: &::Scope<'scope>,
                       depth: usize,
                       into_iter: I,
                       consumer: C) -> C::Result
    where I: IntoIterator,
          I::Item: Send + 'scope,
          C: UnindexedConsumer<I::Item> + 'scope,
          C::Result: 'scope,
{
    let max = if depth == 0 { 1024 } else { depth };
    let min = max / 2;

    let mut queue = VecDeque::with_capacity(max);
    let mut left = consumer.split_off_left().into_folder().complete();
    let mut iter = into_iter.into_iter();
    let mut empty = false;

    while !empty {
        // Draw up to CHUNK number of items at a time. This prevents
        // us from looping infinitely if there is 1 thread and an
        // infinite sequential iterator (presuming, of course, that
        // the consumer can become `full`).
        while queue.len() < max {
            if consumer.full() {
                empty = true;
                break;
            }

            if let Some(item) = iter.next() {
                let left_consumer = consumer.split_off_left();
                queue.push_back(
                    scope.spawn_future(lazy(move || {
                        Ok::<_, ()>(left_consumer.into_folder().consume(item).complete())
                    })));
            } else {
                empty = true;
                break;
            }
        }

        // Reduce the futures we created.
        let stop_len = if empty { 0 } else { min };
        while queue.len() > stop_len {
            let future = queue.pop_front().unwrap();
            let right = future.rayon_wait().unwrap();
            left = consumer.to_reducer().reduce(left, right);
        }
    }

    left
}

