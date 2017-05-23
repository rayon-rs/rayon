use super::internal::*;
use super::*;

/// The `split` function takes arbitrary data and a closure that knows how to
/// split it, and turns this into a `ParallelIterator`.
pub fn split<D, S>(data: D, splitter: S) -> Split<D, S>
    where D: Send,
          S: Fn(D) -> (D, Option<D>) + Sync
{
    Split {
        data: data,
        splitter: splitter,
    }
}

/// `Split` is a parallel iterator using arbitrary data and a splitting function.
/// This struct is created by the [`split()`] function.
///
/// [`split()`]: fn.split.html
pub struct Split<D, S> {
    data: D,
    splitter: S,
}

impl<D, S> ParallelIterator for Split<D, S>
    where D: Send,
          S: Fn(D) -> (D, Option<D>) + Sync + Send
{
    type Item = D;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitProducer {
            data: self.data,
            splitter: &self.splitter,
        };
        bridge_unindexed(producer, consumer)
    }
}

struct SplitProducer<'a, D, S: 'a> {
    data: D,
    splitter: &'a S,
}

impl<'a, D, S> UnindexedProducer for SplitProducer<'a, D, S>
    where D: Send,
          S: Fn(D) -> (D, Option<D>) + Sync
{
    type Item = D;

    fn split(mut self) -> (Self, Option<Self>) {
        let splitter = self.splitter;
        let (left, right) = splitter(self.data);
        self.data = left;
        (self,
         right.map(|data| {
                       SplitProducer {
                           data: data,
                           splitter: splitter,
                       }
                   }))
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        folder.consume(self.data)
    }
}
