//! The rayon prelude imports the various `ParallelIterator` traits.
//! The intention is that one can include `use rayon::prelude::*` and
//! have easy access to the various traits and methods you will need.

pub use iter::FromParallelIterator;
pub use iter::IntoParallelIterator;
pub use iter::IntoParallelRefIterator;
pub use iter::IntoParallelRefMutIterator;
pub use iter::IndexedParallelIterator;
pub use iter::ParallelExtend;
pub use iter::ParallelIterator;
pub use slice::ParallelSlice;
pub use slice::ParallelSliceMut;
pub use str::ParallelString;
