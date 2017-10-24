use iter::internal::*;
use iter::*;

pub fn once<T: Send>(item: T) -> Once<T> {
    Once { item: item }
}

#[derive(Clone, Debug)]
pub struct Once<T: Send> {
    item: T,
}

impl<T: Send> ParallelIterator for Once<T> {
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        self.drive(consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(1)
    }
}

impl<T: Send> IndexedParallelIterator for Once<T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        consumer.into_folder().consume(self.item).complete()
    }

    fn len(&mut self) -> usize {
        1
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        // Let `OptionProducer` handle it.
        Some(self.item).into_par_iter().with_producer(callback)
    }
}
