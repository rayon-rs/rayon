use super::internal::*;
use super::*;
use std::cmp;
use std::iter;

pub struct Zip<A: IndexedParallelIterator, B: IndexedParallelIterator> {
    a: A,
    b: B,
}

/// Create a new `Zip` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<A, B>(a: A, b: B) -> Zip<A, B>
    where A: IndexedParallelIterator<Scheduler = DefaultScheduler>,
          B: IndexedParallelIterator<Scheduler = DefaultScheduler>,
{
    Zip { a: a, b: b }
}

impl<A, B> ParallelIterator for Zip<A, B>
    where A: IndexedParallelIterator,
          B: IndexedParallelIterator
{
    type Item = (A::Item, B::Item);
    type Scheduler = DefaultScheduler;

    fn drive_unindexed<C, S>(self, consumer: C, scheduler: S) -> C::Result
        where C: UnindexedConsumer<Self::Item>, S: Scheduler,
    {
        scheduler.execute(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<A, B> IndexedParallelIterator for Zip<A, B>
    where A: IndexedParallelIterator,
          B: IndexedParallelIterator
{
    fn drive<C, S>(self, consumer: C, scheduler: S) -> C::Result
        where C: Consumer<Self::Item>, S: Scheduler,
    {
        scheduler.execute(self, consumer)
    }

    fn len(&mut self) -> usize {
        cmp::min(self.a.len(), self.b.len())
    }

    fn with_producer<CB, S>(self, callback: CB, scheduler: S) -> CB::Output
        where CB: ProducerCallback<Self::Item>, S: Scheduler,
    {
        return self.a.with_producer(CallbackA {
                                        callback: callback,
                                        b: self.b,
                                    }, scheduler);

        struct CallbackA<CB, B> {
            callback: CB,
            b: B,
        }

        impl<CB, A_ITEM, B> ProducerCallback<A_ITEM> for CallbackA<CB, B>
            where B: IndexedParallelIterator,
                  CB: ProducerCallback<(A_ITEM, B::Item)>
        {
            type Output = CB::Output;

            fn callback<A, S>(self, a_producer: A, scheduler: S) -> Self::Output
                where A: Producer<Item = A_ITEM>, S: Scheduler,
            {
                return self.b.with_producer(CallbackB {
                                                a_producer: a_producer,
                                                callback: self.callback,
                                            }, scheduler);
            }
        }

        struct CallbackB<CB, A> {
            a_producer: A,
            callback: CB,
        }

        impl<CB, A, B_ITEM> ProducerCallback<B_ITEM> for CallbackB<CB, A>
            where A: Producer,
                  CB: ProducerCallback<(A::Item, B_ITEM)>
        {
            type Output = CB::Output;

            fn callback<B, S>(self, b_producer: B, scheduler: S) -> Self::Output
                where B: Producer<Item = B_ITEM>, S: Scheduler,
            {
                self.callback.callback(ZipProducer {
                                           a: self.a_producer,
                                           b: b_producer,
                                       }, scheduler)
            }
        }

    }
}

/// ////////////////////////////////////////////////////////////////////////

struct ZipProducer<A: Producer, B: Producer> {
    a: A,
    b: B,
}

impl<A: Producer, B: Producer> Producer for ZipProducer<A, B> {
    type Item = (A::Item, B::Item);
    type IntoIter = iter::Zip<A::IntoIter, B::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        self.a.into_iter().zip(self.b.into_iter())
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (a_left, a_right) = self.a.split_at(index);
        let (b_left, b_right) = self.b.split_at(index);
        (ZipProducer {
             a: a_left,
             b: b_left,
         },
         ZipProducer {
             a: a_right,
             b: b_right,
         })
    }
}
