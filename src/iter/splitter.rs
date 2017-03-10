use super::internal::*;
use super::*;

/// The `split` function takes arbitrary data and a closure that knows how to
/// split it, and turns this into a `ParallelIterator`.
pub fn split<DATA, SPLITTER>(data: DATA, splitter: SPLITTER) -> ParallelSplit<DATA, SPLITTER>
    where DATA: Send,
          SPLITTER: Fn(DATA) -> (DATA, Option<DATA>) + Sync
{
    ParallelSplit {
        data: data,
        splitter: splitter,
    }
}

pub struct ParallelSplit<DATA, SPLITTER> {
    data: DATA,
    splitter: SPLITTER,
}

impl<DATA, SPLITTER> ParallelIterator for ParallelSplit<DATA, SPLITTER>
    where DATA: Send,
          SPLITTER: Fn(DATA) -> (DATA, Option<DATA>) + Sync
{
    type Item = DATA;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = ParallelSplitProducer {
            data: self.data,
            splitter: &self.splitter,
        };
        bridge_unindexed(producer, consumer)
    }
}

struct ParallelSplitProducer<'a, DATA, SPLITTER: 'a> {
    data: DATA,
    splitter: &'a SPLITTER,
}

impl<'a, DATA, SPLITTER> UnindexedProducer for ParallelSplitProducer<'a, DATA, SPLITTER>
    where DATA: Send,
          SPLITTER: Fn(DATA) -> (DATA, Option<DATA>) + Sync
{
    type Item = DATA;

    fn split(mut self) -> (Self, Option<Self>) {
        let splitter = self.splitter;
        let (left, right) = splitter(self.data);
        self.data = left;
        (self, right.map(|data| {
            ParallelSplitProducer {
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
