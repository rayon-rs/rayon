use super::internal::*;
use super::*;

pub struct Weight<M> {
    base: M,
    weight: f64,
}

impl<M> Weight<M> {
    pub fn new(base: M, weight: f64) -> Weight<M> {
        Weight {
            base: base,
            weight: weight,
        }
    }
}

impl<M> ParallelIterator for Weight<M>
    where M: ParallelIterator
{
    type Item = M::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer1 = WeightConsumer::new(consumer, self.weight);
        self.base.drive_unindexed(consumer1)
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}

impl<M: BoundedParallelIterator> BoundedParallelIterator for Weight<M> {
    fn upper_bound(&mut self) -> usize {
        self.base.upper_bound()
    }

    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        let consumer1: WeightConsumer<C> = WeightConsumer::new(consumer, self.weight);
        self.base.drive(consumer1)
    }
}

impl<M: ExactParallelIterator> ExactParallelIterator for Weight<M> {
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M: IndexedParallelIterator> IndexedParallelIterator for Weight<M> {
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback {
            weight: self.weight,
            callback: callback,
        });

        struct Callback<CB> {
            weight: f64,
            callback: CB,
        }

        impl<I, CB> ProducerCallback<I> for Callback<CB>
            where CB: ProducerCallback<I>
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> Self::Output
                where P: Producer<Item = I>
            {
                self.callback.callback(WeightProducer {
                    base: base,
                    weight: self.weight,
                })
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

struct WeightProducer<P> {
    base: P,
    weight: f64,
}

impl<P: Producer> Producer for WeightProducer<P> {
    type Item = P::Item;
    type IntoIter = P::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.base.into_iter()
    }

    fn weighted(&self) -> bool {
        true
    }

    fn cost(&mut self, len: usize) -> f64 {
        self.base.cost(len) * self.weight
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (WeightProducer {
             base: left,
             weight: self.weight,
         },
         WeightProducer {
             base: right,
             weight: self.weight,
         })
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct WeightConsumer<C> {
    base: C,
    weight: f64,
}

impl<C> WeightConsumer<C> {
    fn new(base: C, weight: f64) -> WeightConsumer<C> {
        WeightConsumer {
            base: base,
            weight: weight,
        }
    }
}

impl<C, I> Consumer<I> for WeightConsumer<C>
    where C: Consumer<I>
{
    type Folder = C::Folder;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn weighted(&self) -> bool {
        true
    }

    fn cost(&mut self, cost: f64) -> f64 {
        self.base.cost(cost) * self.weight
    }

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (WeightConsumer::new(left, self.weight), WeightConsumer::new(right, self.weight), reducer)
    }

    fn into_folder(self) -> C::Folder {
        self.base.into_folder()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<C, I> UnindexedConsumer<I> for WeightConsumer<C>
    where C: UnindexedConsumer<I>
{
    fn split_off_left(&self) -> Self {
        WeightConsumer::new(self.base.split_off_left(), self.weight)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}
