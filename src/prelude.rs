//! The rayon prelude imports the various `ParallelIterator` traits.
//! The intention is that one can include `use rayon::prelude::*` and
//! have easy access to the various traits and methods you will need.

pub use par_iter::IntoParallelIterator;
pub use par_iter::IntoParallelRefIterator;
pub use par_iter::IntoParallelRefMutIterator;
pub use par_iter::ParallelIterator;
pub use par_iter::BoundedParallelIterator;
pub use par_iter::ExactParallelIterator;
pub use par_iter::IndexedParallelIterator;

pub use par_iter::ToParallelChunks;
pub use par_iter::ToParallelChunksMut;
