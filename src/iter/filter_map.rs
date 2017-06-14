use super::internal::*;
use super::*;

/// `FilterMap` creates an iterator that uses `filter_op` to both filter and map elements.
/// This struct is created by the [`filter_map()`] method on [`ParallelIterator`].
///
/// [`filter_map()`]: trait.ParallelIterator.html#method.filter_map
/// [`ParallelIterator`]: trait.ParallelIterator.html
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct FilterMap<I: ParallelIterator, P> {
    base: I,
    filter_op: P,
}

/// Create a new `FilterMap` iterator.
///
/// NB: a free fn because it is NOT part of the end-user API.
pub fn new<I, P>(base: I, filter_op: P) -> FilterMap<I, P>
    where I: ParallelIterator
{
    FilterMap {
        base: base,
        filter_op: filter_op,
    }
}

impl<I, P, R> ParallelIterator for FilterMap<I, P>
    where I: ParallelIterator,
          P: Fn(I::Item) -> Option<R> + Sync + Send,
          R: Send
{
    type Item = R;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let consumer = FilterMapConsumer::new(consumer, &self.filter_op);
        self.base.drive_unindexed(consumer)
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Consumer implementation

struct FilterMapConsumer<'p, C, P: 'p> {
    base: C,
    filter_op: &'p P,
}

impl<'p, C, P: 'p> FilterMapConsumer<'p, C, P> {
    fn new(base: C, filter_op: &'p P) -> Self {
        FilterMapConsumer {
            base: base,
            filter_op: filter_op,
        }
    }
}

impl<'p, T, U, C, P> Consumer<T> for FilterMapConsumer<'p, C, P>
    where C: Consumer<U>,
          P: Fn(T) -> Option<U> + Sync + 'p
{
    type Folder = FilterMapFolder<'p, C::Folder, P>;
    type Reducer = C::Reducer;
    type Result = C::Result;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left, right, reducer) = self.base.split_at(index);
        (FilterMapConsumer::new(left, self.filter_op),
         FilterMapConsumer::new(right, self.filter_op),
         reducer)
    }

    fn into_folder(self) -> Self::Folder {
        let base = self.base.into_folder();
        FilterMapFolder {
            base: base,
            filter_op: self.filter_op,
        }
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}

impl<'p, T, U, C, P> UnindexedConsumer<T> for FilterMapConsumer<'p, C, P>
    where C: UnindexedConsumer<U>,
          P: Fn(T) -> Option<U> + Sync + 'p
{
    fn split_off_left(&self) -> Self {
        FilterMapConsumer::new(self.base.split_off_left(), &self.filter_op)
    }

    fn to_reducer(&self) -> Self::Reducer {
        self.base.to_reducer()
    }
}

struct FilterMapFolder<'p, C, P: 'p> {
    base: C,
    filter_op: &'p P,
}

impl<'p, T, U, C, P> Folder<T> for FilterMapFolder<'p, C, P>
    where C: Folder<U>,
          P: Fn(T) -> Option<U> + Sync + 'p
{
    type Result = C::Result;

    fn consume(self, item: T) -> Self {
        let filter_op = self.filter_op;
        if let Some(mapped_item) = filter_op(item) {
            let base = self.base.consume(mapped_item);
            FilterMapFolder {
                base: base,
                filter_op: filter_op,
            }
        } else {
            self
        }
    }

    fn complete(self) -> C::Result {
        self.base.complete()
    }

    fn full(&self) -> bool {
        self.base.full()
    }
}
