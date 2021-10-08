use super::{
    plumbing::{bridge, Consumer, Producer, ProducerCallback, UnindexedConsumer},
    IndexedParallelIterator, ParallelIterator,
};

/// TODO
pub fn concat<I>(iters: Vec<I>) -> Concat<I>
where
    I: IndexedParallelIterator,
{
    Concat(iters)
}

/// TODO
#[derive(Debug)]
pub struct Concat<I>(Vec<I>);

impl<I> ParallelIterator for Concat<I>
where
    I: IndexedParallelIterator,
{
    type Item = I::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<I> IndexedParallelIterator for Concat<I>
where
    I: IndexedParallelIterator,
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    fn len(&self) -> usize {
        self.0.iter().map(IndexedParallelIterator::len).sum()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        struct Collector<P1> {
            len: usize,
            prods: Vec<(usize, P1)>,
        }

        impl<'a, T, P1> ProducerCallback<T> for &'a mut Collector<P1>
        where
            P1: Producer<Item = T>,
        {
            type Output = ();

            fn callback<P2>(self, producer: P2) -> Self::Output
            where
                P2: Producer<Item = T>,
            {
                // TODO: unify P1 and P2
                self.prods.push((self.len, producer));
            }
        }

        let mut collector = Collector {
            len: 0,
            prods: Vec::new(),
        };

        for iter in self.0 {
            collector.len += iter.len();

            iter.with_producer(&mut collector);
        }

        callback.callback(Concat(collector.prods))
    }
}

impl<P> Producer for Concat<(usize, P)>
where
    P: Producer,
{
    type Item = P::Item;
    type IntoIter = Concat<P::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        Concat(
            self.0
                .into_iter()
                .map(|(_len, prod)| prod.into_iter())
                .collect(),
        )
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        // TODO: binary search
        if let Some(pos) = self.0.iter().position(|(len, _prod)| *len > index) {
            let mut lhs = self.0;
            let mut rhs = lhs.split_off(pos);

            let lhs_len = lhs.last().map_or(0, |(len, _prod)| *len);

            let (rhs_len, rhs_prod) = rhs.remove(0); // TODO: deque

            let (lhs_prod, rhs_prod) = rhs_prod.split_at(index - lhs_len);

            lhs.push((index, lhs_prod));
            rhs.insert(0, (rhs_len - index, rhs_prod)); // TODO: deque

            (Concat(lhs), Concat(rhs))
        } else {
            (self, Concat(Vec::new()))
        }
    }
}

impl<I> Iterator for Concat<I>
where
    I: Iterator + DoubleEndedIterator + ExactSizeIterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let first = self.0.first_mut()?;

            match first.next() {
                Some(item) => return Some(item),
                None => {
                    self.0.remove(0); // TODO: deque
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<I> DoubleEndedIterator for Concat<I>
where
    I: Iterator + DoubleEndedIterator + ExactSizeIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            let last = self.0.last_mut()?;

            match last.next_back() {
                Some(item) => return Some(item),
                None => {
                    self.0.pop();
                }
            }
        }
    }
}

impl<I> ExactSizeIterator for Concat<I>
where
    I: Iterator + DoubleEndedIterator + ExactSizeIterator,
{
    fn len(&self) -> usize {
        self.0.iter().map(ExactSizeIterator::len).sum()
    }
}
