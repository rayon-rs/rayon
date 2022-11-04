use super::plumbing;
use super::private::Residual;
use super::private::Try;
use super::IndexedParallelIterator;
use std::ops::ControlFlow;
use std::ops::ControlFlow::*;

/// Fills the slice with the contents of the parallel iterator, fallibly.
///
/// This is called by `IndexedParallelIterator::try_fill_slice`.
pub(crate) fn try_fill_slice<I, T, R>(pi: I, target: &mut [T]) -> R::TryType
where
    I: IndexedParallelIterator,
    I::Item: Try<Output = T, Residual = R>,
    T: Send,
    R: Residual<()> + Send,
{
    assert_eq!(
        pi.len(),
        target.len(),
        "slice length not equal to iterator length"
    );

    match pi.drive(Consumer { target }) {
        Continue(()) => R::TryType::from_output(()),
        Break(residual) => R::TryType::from_residual(residual),
    }
}

struct Consumer<'target, T> {
    target: &'target mut [T],
}

impl<'target, Item, T, R> plumbing::Consumer<Item> for Consumer<'target, T>
where
    Item: Try<Output = T, Residual = R>,
    T: Send,
    R: Send,
{
    type Folder = Folder<'target, T, R>;
    type Reducer = Reducer;
    type Result = ControlFlow<R>;

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (a, b) = self.target.split_at_mut(index);
        let a = Self { target: a };
        let b = Self { target: b };
        (a, b, Reducer)
    }
    fn into_folder(self) -> Self::Folder {
        Folder(Continue(self.target))
    }
    fn full(&self) -> bool {
        self.target.is_empty()
    }
}

struct Folder<'target, T, R>(ControlFlow<R, &'target mut [T]>);

impl<'target, Item, T, R> plumbing::Folder<Item> for Folder<'target, T, R>
where
    Item: Try<Output = T, Residual = R>,
{
    type Result = ControlFlow<R>;

    fn consume(self, item: Item) -> Self {
        match (self.0, item.branch()) {
            (Continue([reference, rest @ ..]), Continue(item)) => {
                *reference = item;
                Self(Continue(rest))
            }
            (Break(residual), _) | (_, Break(residual)) => Self(Break(residual)),
            (Continue([]), _) => panic!("too many values given to folder"),
        }
    }
    fn complete(self) -> Self::Result {
        let target = self.0?;
        debug_assert!(target.is_empty());
        Continue(())
    }
    fn full(&self) -> bool {
        matches!(&self.0, Continue(&mut []) | Break(_))
    }
}

struct Reducer;

impl<R> plumbing::Reducer<ControlFlow<R>> for Reducer {
    fn reduce(self, left: ControlFlow<R>, right: ControlFlow<R>) -> ControlFlow<R> {
        left?;
        right
    }
}

#[cfg(test)]
mod tests {
    use crate::iter::IndexedParallelIterator;
    use crate::iter::IntoParallelIterator;
    use crate::iter::ParallelIterator;

    #[test]
    fn ok() {
        let mut res = vec![0; 1024];
        (0..1024)
            .into_par_iter()
            .map(Ok::<usize, ()>)
            .try_fill_slice(&mut res)
            .unwrap();
        for (i, v) in res.into_iter().enumerate() {
            assert_eq!(i, v);
        }
    }

    #[test]
    fn one_error() {
        let mut buf = vec![0; 1025];
        let res = (0..1024)
            .into_par_iter()
            .map(Ok)
            .chain([Err(())])
            .try_fill_slice(&mut buf);
        assert_eq!(res, Err(()));
    }
}
