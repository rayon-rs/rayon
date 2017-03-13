use super::internal::*;
use super::*;

pub struct Weight<I> {
    base: I,
    weight: f64,
}

impl<I> Weight<I> {
    pub fn new(base: I, weight: f64) -> Weight<I> {
        Weight {
            base: base,
            weight: weight,
        }
    }
}

impl<I> ParallelIterator for Weight<I>
    where I: ParallelIterator
{
    type Item = I::Item;

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

impl<I: BoundedParallelIterator> BoundedParallelIterator for Weight<I> {
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

impl<I: ExactParallelIterator> ExactParallelIterator for Weight<I> {
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<I: IndexedParallelIterator> IndexedParallelIterator for Weight<I> {
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

        impl<T, CB> ProducerCallback<T> for Callback<CB>
            where CB: ProducerCallback<T>
        {
            type Output = CB::Output;

            fn callback<P>(self, base: P) -> Self::Output
                where P: Producer<Item = T>
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

impl<C, T> Consumer<T> for WeightConsumer<C>
    where C: Consumer<T>
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

impl<C, T> UnindexedConsumer<T> for WeightConsumer<C>
    where C: UnindexedConsumer<T>
{
    fn split_off_left(&self) -> Self {
        WeightConsumer::new(self.base.split_off_left(), self.weight)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

