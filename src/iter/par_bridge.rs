use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::current_num_threads;
use crate::iter::plumbing::{bridge_unindexed, Folder, UnindexedConsumer, UnindexedProducer};
use crate::iter::ParallelIterator;

/// Conversion trait to convert an `Iterator` to a `ParallelIterator`.
///
/// This creates a "bridge" from a sequential iterator to a parallel one, by distributing its items
/// across the Rayon thread pool. This has the advantage of being able to parallelize just about
/// anything, but the resulting `ParallelIterator` can be less efficient than if you started with
/// `par_iter` instead. However, it can still be useful for iterators that are difficult to
/// parallelize by other means, like channels or file or network I/O.
///
/// The resulting iterator is not guaranteed to keep the order of the original iterator.
///
/// # Examples
///
/// To use this trait, take an existing `Iterator` and call `par_bridge` on it. After that, you can
/// use any of the `ParallelIterator` methods:
///
/// ```
/// use rayon::iter::ParallelBridge;
/// use rayon::prelude::ParallelIterator;
/// use std::sync::mpsc::channel;
///
/// let rx = {
///     let (tx, rx) = channel();
///
///     tx.send("one!");
///     tx.send("two!");
///     tx.send("three!");
///
///     rx
/// };
///
/// let mut output: Vec<&'static str> = rx.into_iter().par_bridge().collect();
/// output.sort_unstable();
///
/// assert_eq!(&*output, &["one!", "three!", "two!"]);
/// ```
pub trait ParallelBridge: Sized {
    /// Creates a bridge from this type to a `ParallelIterator`.
    fn par_bridge(self) -> IterBridge<Self>;
}

impl<T: Iterator + Send> ParallelBridge for T
where
    T::Item: Send,
{
    fn par_bridge(self) -> IterBridge<Self> {
        IterBridge { iter: self }
    }
}

/// `IterBridge` is a parallel iterator that wraps a sequential iterator.
///
/// This type is created when using the `par_bridge` method on `ParallelBridge`. See the
/// [`ParallelBridge`] documentation for details.
///
/// [`ParallelBridge`]: trait.ParallelBridge.html
#[derive(Debug, Clone)]
pub struct IterBridge<Iter> {
    iter: Iter,
}

impl<Iter: Iterator + Send> ParallelIterator for IterBridge<Iter>
where
    Iter::Item: Send,
{
    type Item = Iter::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let split_count = AtomicUsize::new(current_num_threads());

        let iter = Mutex::new(self.iter.fuse());

        bridge_unindexed(
            IterParallelProducer {
                split_count: &split_count,
                iter: &iter,
            },
            consumer,
        )
    }
}

struct IterParallelProducer<'a, Iter: Iterator> {
    split_count: &'a AtomicUsize,
    iter: &'a Mutex<std::iter::Fuse<Iter>>,
}

// manual clone because T doesn't need to be Clone, but the derive assumes it should be
impl<'a, Iter: Iterator + 'a> Clone for IterParallelProducer<'a, Iter> {
    fn clone(&self) -> Self {
        IterParallelProducer {
            split_count: self.split_count,
            iter: self.iter,
        }
    }
}

impl<'a, Iter: Iterator + Send + 'a> UnindexedProducer for IterParallelProducer<'a, Iter>
where
    Iter::Item: Send,
{
    type Item = Iter::Item;

    fn split(self) -> (Self, Option<Self>) {
        let mut count = self.split_count.load(Ordering::SeqCst);

        loop {
            // Check if the iterator is exhausted
            if let Some(new_count) = count.checked_sub(1) {
                match self.split_count.compare_exchange_weak(
                    count,
                    new_count,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => return (self.clone(), Some(self)),
                    Err(last_count) => count = last_count,
                }
            } else {
                return (self, None);
            }
        }
    }

    fn fold_with<F>(self, mut folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        loop {
            if let Ok(mut iter) = self.iter.lock() {
                if let Some(it) = iter.next() {
                    drop(iter);
                    folder = folder.consume(it);
                    if folder.full() {
                        return folder;
                    }
                } else {
                    return folder;
                }
            } else {
                // any panics from other threads will have been caught by the pool,
                // and will be re-thrown when joined - just exit
                return folder;
            }
        }
    }
}
