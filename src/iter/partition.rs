use super::internal::*;
use super::*;


/// Partitions the items of a parallel iterator into a pair of arbitrary
/// `ParallelExtend` containers.
pub fn partition<I, C, P>(pi: I, predicate: P) -> (C, C)
    where I: ParallelIterator,
          C: Default + ParallelExtend<I::Item>,
          P: Fn(&I::Item) -> bool + Sync
{
    let mut matches = C::default();
    let mut others = C::default();
    {
        // We have no idea what the consumers will look like for these
        // collections' `par_extend`, but we can intercept them in our own
        // `drive_unindexed`.  Start with the left side for matching items:
        let iter = PartitionMatches {
            base: pi,
            others: &mut others,
            predicate: predicate,
        };
        matches.par_extend(iter);
    }
    (matches, others)
}


/// A fake iterator to intercept the `Consumer` for matching items.
struct PartitionMatches<'c, I, C: 'c, P> {
    base: I,
    others: &'c mut C,
    predicate: P,
}

impl<'c, I, C, P> ParallelIterator for PartitionMatches<'c, I, C, P>
    where I: ParallelIterator,
          C: Default + ParallelExtend<I::Item>,
          P: Fn(&I::Item) -> bool + Sync
{
    type Item = I::Item;

    fn drive_unindexed<CM>(self, matches: CM) -> CM::Result
        where CM: UnindexedConsumer<Self::Item>
    {
        let mut result = None;
        {
            // Now it's time to find the consumer for non-matching items
            let iter = PartitionOthers {
                base: self.base,
                matches: matches,
                match_result: &mut result,
                predicate: self.predicate,
            };
            self.others.par_extend(iter);
        }
        result.unwrap()
    }
}


/// A fake iterator to intercept the `Consumer` for non-matching items.
struct PartitionOthers<'r, I, P, CM>
    where I: ParallelIterator,
          CM: UnindexedConsumer<I::Item>,
          CM::Result: 'r
{
    base: I,
    matches: CM,
    match_result: &'r mut Option<CM::Result>,
    predicate: P,
}

impl<'r, I, P, CM> ParallelIterator for PartitionOthers<'r, I, P, CM>
    where I: ParallelIterator,
          P: Fn(&I::Item) -> bool + Sync,
          CM: UnindexedConsumer<I::Item>
{
    type Item = I::Item;

    fn drive_unindexed<CO>(self, others: CO) -> CO::Result
        where CO: UnindexedConsumer<Self::Item>
    {
        // Now that we have two consumers, we can partition the real iterator.
        let consumer = PartitionConsumer {
            matches: self.matches,
            others: others,
            predicate: &self.predicate,
        };

        let result = self.base.drive_unindexed(consumer);
        *self.match_result = Some(result.0);
        result.1
    }
}


/// `Consumer` that partitions into two other `Consumer`s
struct PartitionConsumer<'p, P: 'p, CM, CO> {
    matches: CM,
    others: CO,
    predicate: &'p P,
}

impl<'p, P, T, CM, CO> Consumer<T> for PartitionConsumer<'p, P, CM, CO>
    where P: Fn(&T) -> bool + Sync,
          CM: Consumer<T>,
          CO: Consumer<T>
{
    type Folder = PartitionFolder<'p, P, CM::Folder, CO::Folder>;
    type Reducer = PartitionReducer<CM::Reducer, CO::Reducer>;
    type Result = (CM::Result, CO::Result);

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (matches1, matches2, matches_reducer) = self.matches.split_at(index);
        let (others1, others2, others_reducer) = self.others.split_at(index);

        (PartitionConsumer {
             matches: matches1,
             others: others1,
             ..self
         },
         PartitionConsumer {
             matches: matches2,
             others: others2,
             ..self
         },
         PartitionReducer {
             matches: matches_reducer,
             others: others_reducer,
         })
    }

    fn into_folder(self) -> Self::Folder {
        PartitionFolder {
            matches: self.matches.into_folder(),
            others: self.others.into_folder(),
            predicate: self.predicate,
        }
    }

    fn full(&self) -> bool {
        // don't stop until everyone is full
        self.matches.full() && self.others.full()
    }
}

impl<'p, P, T, CM, CO> UnindexedConsumer<T> for PartitionConsumer<'p, P, CM, CO>
    where P: Fn(&T) -> bool + Sync,
          CM: UnindexedConsumer<T>,
          CO: UnindexedConsumer<T>
{
    fn split_off_left(&self) -> Self {
        PartitionConsumer {
            matches: self.matches.split_off_left(),
            others: self.others.split_off_left(),
            predicate: self.predicate,
        }
    }

    fn to_reducer(&self) -> Self::Reducer {
        PartitionReducer {
            matches: self.matches.to_reducer(),
            others: self.others.to_reducer(),
        }
    }
}


/// `Folder` that partitions into two other `Folder`s
struct PartitionFolder<'p, P: 'p, FM, FO> {
    matches: FM,
    others: FO,
    predicate: &'p P,
}

impl<'p, P, T, FM, FO> Folder<T> for PartitionFolder<'p, P, FM, FO>
    where P: Fn(&T) -> bool + Sync,
          FM: Folder<T>,
          FO: Folder<T>
{
    type Result = (FM::Result, FO::Result);

    fn consume(mut self, item: T) -> Self {
        if (self.predicate)(&item) {
            self.matches = self.matches.consume(item);
        } else {
            self.others = self.others.consume(item);
        }
        self
    }

    fn complete(self) -> Self::Result {
        (self.matches.complete(), self.others.complete())
    }

    fn full(&self) -> bool {
        // don't stop until everyone is full
        self.matches.full() && self.others.full()
    }
}


/// `Reducer` that unzips into two other `Reducer`s
struct PartitionReducer<RM, RO> {
    matches: RM,
    others: RO,
}

impl<T, U, RM, RO> Reducer<(T, U)> for PartitionReducer<RM, RO>
    where RM: Reducer<T>,
          RO: Reducer<U>
{
    fn reduce(self, left: (T, U), right: (T, U)) -> (T, U) {
        (self.matches.reduce(left.0, right.0), self.others.reduce(left.1, right.1))
    }
}
