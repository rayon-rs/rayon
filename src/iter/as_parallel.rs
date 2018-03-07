use crossbeam_deque::{Deque, Stealer, Steal};

use std::thread::yield_now;
use std::sync::{Mutex, TryLockError};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

use iter::*;
use current_num_threads;

/// Conversion trait to convert an `Iterator` to a `ParallelIterator`.
///
/// This needs to be distinct from `IntoParallelIterator` because that trait is already implemented
/// on a few `Iterator`s, like `std::ops::Range`.
pub trait AsParallel {
    /// What is the type of the output `ParallelIterator`?
    type Iter: ParallelIterator<Item = Self::Item>;

    /// What is the `Item` of the output `ParallelIterator`?
    type Item: Send;

    /// Convert this type to a `ParallelIterator`.
    fn as_parallel(self) -> Self::Iter;
}

impl<T: Iterator + Send> AsParallel for T
    where T::Item: Send
{
    type Iter = IterParallel<T>;
    type Item = T::Item;

    fn as_parallel(self) -> Self::Iter {
        IterParallel {
            iter: self,
        }
    }
}

/// `IterParallel` is a parallel iterator that wraps a sequential iterator.
#[derive(Debug)]
pub struct IterParallel<Iter> {
    iter: Iter,
}

impl<Iter: Iterator + Send> ParallelIterator for IterParallel<Iter>
    where Iter::Item: Send
{
    type Item = Iter::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let split_count = AtomicUsize::new(current_num_threads());
        let deque = Deque::new();
        let stealer = deque.stealer();
        let done = AtomicBool::new(false);
        let iter = Mutex::new((self.iter, deque));

        bridge_unindexed(IterParallelProducer {
            split_count: &split_count,
            done: &done,
            iter: &iter,
            items: stealer,
        }, consumer)
    }
}

struct IterParallelProducer<'a, Iter: Iterator + 'a> {
    split_count: &'a AtomicUsize,
    done: &'a AtomicBool,
    iter: &'a Mutex<(Iter, Deque<Iter::Item>)>,
    items: Stealer<Iter::Item>,
}

// manual clone because T doesn't need to be Clone, but the derive assumes it should be
impl<'a, Iter: Iterator + 'a> Clone for IterParallelProducer<'a, Iter> {
    fn clone(&self) -> Self {
        IterParallelProducer {
            split_count: self.split_count,
            done: self.done,
            iter: self.iter,
            items: self.items.clone(),
        }
    }
}

impl<'a, Iter: Iterator + Send + 'a> UnindexedProducer for IterParallelProducer<'a, Iter>
    where Iter::Item: Send
{
    type Item = Iter::Item;

    fn split(self) -> (Self, Option<Self>) {
        let mut count = self.split_count.load(Ordering::SeqCst);

        loop {
            let done = self.done.load(Ordering::SeqCst);
            match count.checked_sub(1) {
                Some(new_count) if !done => {
                    let last_count = self.split_count.compare_and_swap(count, new_count, Ordering::SeqCst);
                    if last_count == count {
                        return (self.clone(), Some(self));
                    } else {
                        count = last_count;
                    }
                }
                _ => {
                    return (self, None);
                }
            }
        }
    }

    fn fold_with<F>(self, mut folder: F) -> F
        where F: Folder<Self::Item>
    {
        loop {
            match self.items.steal() {
                Steal::Data(it) => {
                    folder = folder.consume(it);
                    if folder.full() {
                        return folder;
                    }
                }
                Steal::Empty => {
                    if self.done.load(Ordering::SeqCst) {
                        // the iterator is out of items, no use in continuing
                        return folder;
                    } else {
                        // our cache is out of items, time to load more from the iterator
                        match self.iter.try_lock() {
                            Ok(mut guard) => {
                                let count = current_num_threads();
                                let count = (count * count) * 2;

                                let (ref mut iter, ref deque) = *guard;

                                for _ in 0..count {
                                    if let Some(it) = iter.next() {
                                        deque.push(it);
                                    } else {
                                        self.done.store(true, Ordering::SeqCst);
                                        break;
                                    }
                                }
                            }
                            Err(TryLockError::WouldBlock) => {
                                // someone else has the mutex, just sit tight until it's ready
                                yield_now(); //TODO: use a thread=pool-aware yield? (#548)
                            }
                            Err(TryLockError::Poisoned(_)) => {
                                // TODO: how to handle poison?
                                return folder;
                            }
                        }
                    }
                }
                Steal::Retry => (),
            }
        }
    }
}
