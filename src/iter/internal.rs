//! Internal traits and functions used to implement parallel
//! iteration. These should be considered highly unstable: users of
//! parallel iterators should not need to interact with them directly.
//! See `README.md` for a high-level overview.

use rayon_core::join;
use super::IndexedParallelIterator;

use std::cmp;
use std::usize;

pub trait ProducerCallback<T> {
    type Output;
    fn callback<P>(self, producer: P) -> Self::Output where P: Producer<Item = T>;
}

/// A producer which will produce a fixed number of items N. This is
/// not queryable through the API; the consumer is expected to track
/// it.
pub trait Producer: Send + Sized {
    // Rust issue https://github.com/rust-lang/rust/issues/20671
    // prevents us from declaring the DoubleEndedIterator and
    // ExactSizeIterator constraints on a required IntoIterator trait,
    // so we inline IntoIterator here until that issue is fixed.
    type Item;
    type IntoIter: Iterator<Item = Self::Item> + DoubleEndedIterator + ExactSizeIterator;

    fn into_iter(self) -> Self::IntoIter;

    fn min_len(&self) -> usize {
        1
    }
    fn max_len(&self) -> usize {
        usize::MAX
    }

    /// Split into two producers; one produces items `0..index`, the
    /// other `index..N`. Index must be less than or equal to `N`.
    fn split_at(self, index: usize) -> (Self, Self);

    /// Iterate the producer, feeding each element to `folder`, and
    /// stop when the folder is full (or all elements have been consumed).
    ///
    /// The provided implementation is sufficient for most iterables.
    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        folder.consume_iter(self.into_iter())
    }
}

/// A consumer which consumes items that are fed to it.
pub trait Consumer<Item>: Send + Sized {
    type Folder: Folder<Item, Result = Self::Result>;
    type Reducer: Reducer<Self::Result>;
    type Result: Send;

    /// Divide the consumer into two consumers, one processing items
    /// `0..index` and one processing items from `index..`. Also
    /// produces a reducer that can be used to reduce the results at
    /// the end.
    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer);

    /// Convert the consumer into a folder that can consume items
    /// sequentially, eventually producing a final result.
    fn into_folder(self) -> Self::Folder;

    /// Hint whether this `Consumer` would like to stop processing
    /// further items, e.g. if a search has been completed.
    fn full(&self) -> bool;
}

pub trait Folder<Item>: Sized {
    type Result;

    /// Consume next item and return new sequential state.
    fn consume(self, item: Item) -> Self;

    /// Consume items from the iterator until full, and return new sequential state.
    fn consume_iter<I>(mut self, iter: I) -> Self
        where I: IntoIterator<Item = Item>
    {
        for item in iter {
            self = self.consume(item);
            if self.full() {
                break;
            }
        }
        self
    }

    /// Finish consuming items, produce final result.
    fn complete(self) -> Self::Result;

    /// Hint whether this `Folder` would like to stop processing
    /// further items, e.g. if a search has been completed.
    fn full(&self) -> bool;
}

pub trait Reducer<Result> {
    /// Reduce two final results into one; this is executed after a
    /// split.
    fn reduce(self, left: Result, right: Result) -> Result;
}

/// A stateless consumer can be freely copied.
pub trait UnindexedConsumer<I>: Consumer<I> {
    // The result of split_off_left should be used for the left side of the
    // data it consumes, and the remaining consumer for the right side
    // (this matters for methods like find_first).
    fn split_off_left(&self) -> Self;
    fn to_reducer(&self) -> Self::Reducer;
}

/// An unindexed producer that doesn't know its exact length.
/// (or can't represent its known length in a `usize`)
pub trait UnindexedProducer: Send + Sized {
    type Item;

    /// Split midway into a new producer if possible, otherwise return `None`.
    fn split(self) -> (Self, Option<Self>);

    /// Iterate the producer, feeding each element to `folder`, and
    /// stop when the folder is full (or all elements have been consumed).
    fn fold_with<F>(self, folder: F) -> F where F: Folder<Self::Item>;
}

/// A splitter controls the policy for splitting into smaller work items.
///
/// Thief-splitting is an adaptive policy that starts by splitting into
/// enough jobs for every worker thread, and then resets itself whenever a
/// job is actually stolen into a different thread.
#[derive(Clone, Copy)]
struct Splitter {
    /// The `origin` tracks the ID of the thread that started this job,
    /// so we can tell when we've been stolen to a new thread.
    origin: usize,

    /// The `splits` tell us approximately how many remaining times we'd
    /// like to split this job.  We always just divide it by two though, so
    /// the effective number of pieces will be `next_power_of_two()`.
    splits: usize,
}

impl Splitter {
    #[inline]
    fn thief_id() -> usize {
        // The actual `ID` value is irrelevant.  We're just using its TLS
        // address as a unique thread key, faster than a real thread-id call.
        thread_local!{ static ID: bool = false }
        ID.with(|id| id as *const bool as usize)
    }

    #[inline]
    fn new() -> Splitter {
        Splitter {
            origin: Splitter::thief_id(),
            splits: ::current_num_threads(),
        }
    }

    #[inline]
    fn try(&mut self) -> bool {
        let Splitter { origin, splits } = *self;

        let id = Splitter::thief_id();
        if origin != id {
            // This job was stolen!  Set a new origin and reset the number
            // of desired splits to the thread count, if that's more than
            // we had remaining anyway.
            self.origin = id;
            self.splits = cmp::max(::current_num_threads(), self.splits / 2);
            true
        } else if splits > 0 {
            // We have splits remaining, make it so.
            self.splits /= 2;
            true
        } else {
            // Not stolen, and no more splits -- we're done!
            false
        }
    }
}

/// The length splitter is built on thief-splitting, but additionally takes
/// into account the remaining length of the iterator.
#[derive(Clone, Copy)]
struct LengthSplitter {
    inner: Splitter,

    /// The smallest we're willing to divide into.  Usually this is just 1,
    /// but you can choose a larger working size with `with_min_len()`.
    min: usize,
}

impl LengthSplitter {
    /// Create a new splitter based on lengths.
    ///
    /// The `min` is a hard lower bound.  We'll never split below that, but
    /// of course an iterator might start out smaller already.
    ///
    /// The `max` is an upper bound on the working size, used to determine
    /// the minimum number of times we need to split to get under that limit.
    /// The adaptive algorithm may very well split even further, but never
    /// smaller than the `min`.
    #[inline]
    fn new(min: usize, max: usize, len: usize) -> LengthSplitter {
        let mut splitter = LengthSplitter {
            inner: Splitter::new(),
            min: cmp::max(min, 1),
        };

        // Divide the given length by the max working length to get the minimum
        // number of splits we need to get under that max.  This rounds down,
        // but the splitter actually gives `next_power_of_two()` pieces anyway.
        // e.g. len 12345 / max 100 = 123 min_splits -> 128 pieces.
        let min_splits = len / cmp::max(max, 1);

        // Only update the value if it's not splitting enough already.
        if min_splits > splitter.inner.splits {
            splitter.inner.splits = min_splits;
        }

        splitter
    }

    #[inline]
    fn try(&mut self, len: usize) -> bool {
        // If splitting wouldn't make us too small, try the inner splitter.
        len / 2 >= self.min && self.inner.try()
    }
}

pub fn bridge<I, C>(mut par_iter: I, consumer: C) -> C::Result
    where I: IndexedParallelIterator,
          C: Consumer<I::Item>
{
    let len = par_iter.len();
    return par_iter.with_producer(Callback {
                                      len: len,
                                      consumer: consumer,
                                  });

    struct Callback<C> {
        len: usize,
        consumer: C,
    }

    impl<C, I> ProducerCallback<I> for Callback<C>
        where C: Consumer<I>
    {
        type Output = C::Result;
        fn callback<P>(self, producer: P) -> C::Result
            where P: Producer<Item = I>
        {
            bridge_producer_consumer(self.len, producer, self.consumer)
        }
    }
}

pub fn bridge_producer_consumer<P, C>(len: usize, producer: P, consumer: C) -> C::Result
    where P: Producer,
          C: Consumer<P::Item>
{
    let splitter = LengthSplitter::new(producer.min_len(), producer.max_len(), len);
    return helper(len, splitter, producer, consumer);

    fn helper<P, C>(len: usize, mut splitter: LengthSplitter, producer: P, consumer: C) -> C::Result
        where P: Producer,
              C: Consumer<P::Item>
    {
        if consumer.full() {
            consumer.into_folder().complete()
        } else if splitter.try(len) {
            let mid = len / 2;
            let (left_producer, right_producer) = producer.split_at(mid);
            let (left_consumer, right_consumer, reducer) = consumer.split_at(mid);
            let (left_result, right_result) =
                join(|| helper(mid, splitter, left_producer, left_consumer),
                     || helper(len - mid, splitter, right_producer, right_consumer));
            reducer.reduce(left_result, right_result)
        } else {
            producer.fold_with(consumer.into_folder()).complete()
        }
    }
}

pub fn bridge_unindexed<P, C>(producer: P, consumer: C) -> C::Result
    where P: UnindexedProducer,
          C: UnindexedConsumer<P::Item>
{
    let splitter = Splitter::new();
    bridge_unindexed_producer_consumer(splitter, producer, consumer)
}

fn bridge_unindexed_producer_consumer<P, C>(mut splitter: Splitter,
                                            producer: P,
                                            consumer: C)
                                            -> C::Result
    where P: UnindexedProducer,
          C: UnindexedConsumer<P::Item>
{
    if consumer.full() {
        consumer.into_folder().complete()
    } else if splitter.try() {
        match producer.split() {
            (left_producer, Some(right_producer)) => {
                let (reducer, left_consumer, right_consumer) =
                    (consumer.to_reducer(), consumer.split_off_left(), consumer);
                let bridge = bridge_unindexed_producer_consumer;
                let (left_result, right_result) =
                    join(|| bridge(splitter, left_producer, left_consumer),
                         || bridge(splitter, right_producer, right_consumer));
                reducer.reduce(left_result, right_result)
            }
            (producer, None) => producer.fold_with(consumer.into_folder()).complete(),
        }
    } else {
        producer.fold_with(consumer.into_folder()).complete()
    }
}
