use super::internal::*;
use super::*;


/// Unzips the items of a parallel iterator into a pair of arbitrary
/// `ParallelExtend` containers.
pub fn unzip<I, A, B, FromA, FromB>(pi: I) -> (FromA, FromB)
    where I: ParallelIterator<Item = (A, B)>,
          FromA: Default + ParallelExtend<A>,
          FromB: Default + ParallelExtend<B>,
          A: Send,
          B: Send
{
    let mut a = FromA::default();
    let mut b = FromB::default();
    {
        // We have no idea what the consumers will look like for these
        // collections' `par_extend`, but we can intercept them in our own
        // `drive_unindexed`.  Start with the left side, type `A`:
        let iter = UnzipA {
            base: pi,
            b: &mut b,
        };
        a.par_extend(iter);
    }
    (a, b)
}

/// Unzip an `IndexedParallelIterator` into two arbitrary `Consumer`s.
pub fn unzip_indexed<I, A, B, CA, CB>(pi: I, left: CA, right: CB) -> (CA::Result, CB::Result)
    where I: IndexedParallelIterator<Item = (A, B)>,
          CA: Consumer<A>,
          CB: Consumer<B>,
          A: Send,
          B: Send
{
    let consumer = UnzipConsumer {
        left: left,
        right: right,
    };
    pi.drive(consumer)
}

/// Unzip a `ParallelIterator` into two arbitrary `UnindexedConsumer`s.
pub fn unzip_unindexed<I, A, B, CA, CB>(pi: I, left: CA, right: CB) -> (CA::Result, CB::Result)
    where I: ParallelIterator<Item = (A, B)>,
          CA: UnindexedConsumer<A>,
          CB: UnindexedConsumer<B>,
          A: Send,
          B: Send
{
    let consumer = UnzipConsumer {
        left: left,
        right: right,
    };
    pi.drive_unindexed(consumer)
}


/// A fake iterator to intercept the `Consumer` for type `A`.
struct UnzipA<'b, I, FromB: 'b> {
    base: I,
    b: &'b mut FromB,
}

impl<'b, I, A, B, FromB> ParallelIterator for UnzipA<'b, I, FromB>
    where I: ParallelIterator<Item = (A, B)>,
          FromB: Default + ParallelExtend<B>,
          A: Send,
          B: Send
{
    type Item = A;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let mut result = None;
        {
            // Now it's time to find the consumer for type `B`
            let iter = UnzipB {
                base: self.base,
                left_consumer: consumer,
                left_result: &mut result,
            };
            self.b.par_extend(iter);
        }
        result.unwrap()
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}

/// A fake iterator to intercept the `Consumer` for type `B`.
struct UnzipB<'r, I, A, CA>
    where CA: UnindexedConsumer<A>,
          CA::Result: 'r
{
    base: I,
    left_consumer: CA,
    left_result: &'r mut Option<CA::Result>,
}

impl<'r, I, A, B, CA> ParallelIterator for UnzipB<'r, I, A, CA>
    where I: ParallelIterator<Item = (A, B)>,
          CA: UnindexedConsumer<A>,
          A: Send,
          B: Send
{
    type Item = B;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        // Now that we have two consumers, we can unzip the real iterator.
        let result = unzip_unindexed(self.base, self.left_consumer, consumer);
        *self.left_result = Some(result.0);
        result.1
    }

    fn opt_len(&mut self) -> Option<usize> {
        self.base.opt_len()
    }
}


/// `Consumer` that unzips into two other `Consumer`s
struct UnzipConsumer<CA, CB> {
    left: CA,
    right: CB,
}

impl<A, B, CA, CB> Consumer<(A, B)> for UnzipConsumer<CA, CB>
    where CA: Consumer<A>,
          CB: Consumer<B>
{
    type Folder = UnzipFolder<CA::Folder, CB::Folder>;
    type Reducer = UnzipReducer<CA::Reducer, CB::Reducer>;
    type Result = (CA::Result, CB::Result);

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (left1, left2, left_reducer) = self.left.split_at(index);
        let (right1, right2, right_reducer) = self.right.split_at(index);

        (UnzipConsumer {
             left: left1,
             right: right1,
         },
         UnzipConsumer {
             left: left2,
             right: right2,
         },
         UnzipReducer {
             left: left_reducer,
             right: right_reducer,
         })
    }

    fn into_folder(self) -> Self::Folder {
        UnzipFolder {
            left: self.left.into_folder(),
            right: self.right.into_folder(),
        }
    }

    fn full(&self) -> bool {
        // don't stop until everyone is full
        self.left.full() && self.right.full()
    }
}

impl<A, B, CA, CB> UnindexedConsumer<(A, B)> for UnzipConsumer<CA, CB>
    where CA: UnindexedConsumer<A>,
          CB: UnindexedConsumer<B>
{
    fn split_off_left(&self) -> Self {
        UnzipConsumer {
            left: self.left.split_off_left(),
            right: self.right.split_off_left(),
        }
    }

    fn to_reducer(&self) -> Self::Reducer {
        UnzipReducer {
            left: self.left.to_reducer(),
            right: self.right.to_reducer(),
        }
    }
}


/// `Folder` that unzips into two other `Folder`s
struct UnzipFolder<FA, FB> {
    left: FA,
    right: FB,
}

impl<A, B, FA, FB> Folder<(A, B)> for UnzipFolder<FA, FB>
    where FA: Folder<A>,
          FB: Folder<B>
{
    type Result = (FA::Result, FB::Result);

    fn consume(self, (left, right): (A, B)) -> Self {
        UnzipFolder {
            left: self.left.consume(left),
            right: self.right.consume(right),
        }
    }

    fn complete(self) -> Self::Result {
        (self.left.complete(), self.right.complete())
    }

    fn full(&self) -> bool {
        // don't stop until everyone is full
        self.left.full() && self.right.full()
    }
}


/// `Reducer` that unzips into two other `Reducer`s
struct UnzipReducer<RA, RB> {
    left: RA,
    right: RB,
}

impl<A, B, RA, RB> Reducer<(A, B)> for UnzipReducer<RA, RB>
    where RA: Reducer<A>,
          RB: Reducer<B>
{
    fn reduce(self, left: (A, B), right: (A, B)) -> (A, B) {
        (self.left.reduce(left.0, right.0), self.right.reduce(left.1, right.1))
    }
}
