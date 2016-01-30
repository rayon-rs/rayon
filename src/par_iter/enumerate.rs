use super::*;
use super::internal::*;
use super::util::PhantomType;

pub struct Enumerate<M> {
    base: M,
}

impl<M> Enumerate<M> {
    pub fn new(base: M) -> Enumerate<M> {
        Enumerate { base: base }
    }
}

impl<M> ParallelIterator for Enumerate<M>
    where M: IndexedParallelIterator,
{
    type Item = (usize, M::Item);

    fn drive_unindexed<'c, C: UnindexedConsumer<'c, Item=Self::Item>>(self,
                                                                       consumer: C,
                                                                       shared: &'c C::Shared)
                                                                       -> C::Result {
        bridge(self, consumer, &shared)
    }
}

unsafe impl<M> BoundedParallelIterator for Enumerate<M>
    where M: IndexedParallelIterator,
{
    fn upper_bound(&mut self) -> usize {
        self.len()
    }

    fn drive<'c, C: Consumer<'c, Item=Self::Item>>(self,
                                                   consumer: C,
                                                   shared: &'c C::Shared)
                                                   -> C::Result {
        bridge(self, consumer, &shared)
    }
}

unsafe impl<M> ExactParallelIterator for Enumerate<M>
    where M: IndexedParallelIterator,
{
    fn len(&mut self) -> usize {
        self.base.len()
    }
}

impl<M> IndexedParallelIterator for Enumerate<M>
    where M: IndexedParallelIterator,
{
    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        return self.base.with_producer(Callback { callback: callback });

        struct Callback<CB> {
            callback: CB,
        }

        impl<ITEM, CB> ProducerCallback<ITEM> for Callback<CB>
            where CB: ProducerCallback<(usize, ITEM)>
        {
            type Output = CB::Output;
            fn callback<'produce, P>(self,
                                     base: P,
                                     shared: &'produce P::Shared)
                                     -> CB::Output
                where P: Producer<'produce, Item=ITEM>
            {
                let producer = EnumerateProducer { base: base,
                                                   offset: 0,
                                                   phantoms: PhantomType::new() };
                self.callback.callback(producer, shared)
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// Producer implementation

pub struct EnumerateProducer<'p, P>
    where P: Producer<'p>,
{
    base: P,
    offset: usize,
    phantoms: PhantomType<&'p ()>,
}

impl<'p, P> Producer<'p> for EnumerateProducer<'p, P>
    where P: Producer<'p>
{
    type Item = (usize, P::Item);
    type Shared = P::Shared;

    fn cost(&mut self, shared: &Self::Shared, items: usize) -> f64 {
        self.base.cost(shared, items) // enumerating is basically free
    }

    unsafe fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.base.split_at(index);
        (EnumerateProducer { base: left,
                             offset: self.offset,
                             phantoms: PhantomType::new() },
         EnumerateProducer { base: right,
                             offset: self.offset + index,
                             phantoms: PhantomType::new() })
    }

    unsafe fn produce(&mut self, shared: &Self::Shared) -> (usize, P::Item) {
        let item = self.base.produce(shared);
        let index = self.offset;
        self.offset += 1;
        (index, item)
    }
}
