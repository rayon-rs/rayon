use super::{noop::NoopReducer, plumbing, IndexedParallelIterator};

/// Fills the slice with the contents of the parallel iterator.
///
/// This is called by `IndexedParallelIterator::fill_slice`.
pub(super) fn fill_slice<I, T>(pi: I, target: &mut [T])
where
    I: IndexedParallelIterator<Item = T>,
    T: Send,
{
    assert_eq!(
        pi.len(),
        target.len(),
        "slice length not equal to iterator length"
    );

    pi.drive(Consumer { target })
}

struct Consumer<'target, T> {
    target: &'target mut [T],
}

impl<'target, T> plumbing::Consumer<T> for Consumer<'target, T>
where
    T: Send,
{
    type Folder = Folder<'target, T>;
    type Reducer = NoopReducer;
    type Result = ();

    fn split_at(self, index: usize) -> (Self, Self, Self::Reducer) {
        let (a, b) = self.target.split_at_mut(index);
        let a = Self { target: a };
        let b = Self { target: b };
        (a, b, NoopReducer)
    }
    fn into_folder(self) -> Self::Folder {
        Folder(self.target)
    }
    fn full(&self) -> bool {
        self.target.is_empty()
    }
}

struct Folder<'slice, T>(&'slice mut [T]);

impl<'slice, T> plumbing::Folder<T> for Folder<'slice, T> {
    type Result = ();

    fn consume(self, item: T) -> Self {
        let (reference, rest) = self
            .0
            .split_first_mut()
            .expect("too many values given to folder");
        *reference = item;
        Self(rest)
    }
    fn complete(self) -> Self::Result {
        debug_assert!(self.0.is_empty());
    }
    fn full(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::iter::IndexedParallelIterator;
    use crate::iter::IntoParallelIterator;

    #[test]
    fn fill() {
        let mut res = vec![0; 1024];
        (0..1024).into_par_iter().fill_slice(&mut res);
        for (i, v) in res.into_iter().enumerate() {
            assert_eq!(i, v);
        }
    }
}
