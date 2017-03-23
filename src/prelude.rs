//! The rayon prelude imports the various `ParallelIterator` traits.
//! The intention is that one can include `use rayon::prelude::*` and
//! have easy access to the various traits and methods you will need.

pub use iter::BoundedParallelIterator;
pub use iter::ExactParallelIterator;
pub use iter::FromParallelIterator;
pub use iter::IntoParallelIterator;
pub use iter::IntoParallelRefIterator;
pub use iter::IntoParallelRefMutIterator;
pub use iter::IndexedParallelIterator;
pub use iter::ParallelIterator;
pub use iter::ToParallelChunks;
pub use iter::ToParallelChunksMut;
pub use slice::{SliceIter, ChunksIter, SliceIterMut, ChunksMutIter};
pub use str::ParallelString;

