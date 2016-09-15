use super::*;
use super::internal::*;

pub struct Threshold<M> {
    base: M,
    threshold: usize,
}

impl<M> Threshold<M> {
    pub fn new(base: M, threshold: usize) -> Threshold<M> {
        Threshold { base: base, threshold: threshold }
    }
}

impl<M> ParallelIterator for Threshold<M>
    where M: ParallelIterator,
{
    type Item = M::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = ThresholdConsumer::new(consumer, self.threshold);
        self.base.drive_unindexed(consumer1)
    }
}

impl<M: BoundedParallelIterator> BoundedParallelIterator for Threshold<M> {
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        let consumer1: ThresholdConsumer<C> = ThresholdConsumer::new(consumer, self.threshold);
        self.base.drive(consumer1)
    }
}

impl<M: ExactParallelIterator> ExactParallelIterator for Threshold<M> {
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M: IndexedParallelIterator> IndexedParallelIterator for Threshold<M> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback { threshold: self.threshold, callback: callback });

        struct Callback<CB> {
            threshold: usize,
            callback: CB
        }

        impl<ITEM,CB> ProducerCallback<ITEM> for Callback<CB>
            where CB: ProducerCallback<ITEM>
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> Self::Output
                where P: Producer<Item=ITEM>
            {
                self.callback.callback(ThresholdProducer { base: base,
                                                        threshold: self.threshold })
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct ThresholdProducer<P> {
    base: P,
    threshold: usize,
}

impl<P: Producer> Producer for ThresholdProducer<P> {
    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (ThresholdProducer { base: left, threshold: self.threshold },
         ThresholdProducer { base: right, threshold: self.threshold })
    }
}

impl<P: Producer> IntoIterator for ThresholdProducer<P> {
    type Item = P::Item;
    type IntoIter = P::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter()
    }
}

///////////////////////////////////////////////////////////////////////////
// Consumer implementation

struct ThresholdConsumer<C> {
    base: C,
    threshold: usize,
}

impl<C> ThresholdConsumer<C> {
    fn new(base: C, threshold: usize) -> ThresholdConsumer<C> {
        ThresholdConsumer { base: base, threshold: threshold }
    }
}

impl<C, ITEM> Consumer<ITEM> for ThresholdConsumer<C>
    where C: Consumer<ITEM>,
{
    type Folder = C::Folder;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn sequential_threshold(&self) -> usize {
        self.threshold
    }

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (ThresholdConsumer::new(left, self.threshold),
         ThresholdConsumer::new(right, self.threshold),
         reducer)
    }

    fn into_folder(self) -> C::Folder {
        self.base.into_folder()
    }
}

impl<C, ITEM> UnindexedConsumer<ITEM> for ThresholdConsumer<C>
    where C: UnindexedConsumer<ITEM>
{
    fn split_off(&self) -> Self {
        ThresholdConsumer::new(self.base.split_off(), self.threshold)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}
