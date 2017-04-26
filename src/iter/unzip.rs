use super::internal::*;
use super::*;


/// Unzip an `IndexedParallelIterator` into two arbitrary `Consumer`s.
pub fn unzip_indexed<I, A, B, CA, CB>(pi: I, left: CA, right: CB)
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
    pi.drive(consumer);
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
