//! The `ParallelIterator` module makes it easy to write parallel programs
//! using an iterator-style interface. To get access to all the methods you
//! want, the easiest is to write `use rayon::prelude::*;` at the top of your
//! module, which will import the various traits and methods you need.

pub use rayon_core::par_iter::IntoParallelIterator;
pub use rayon_core::par_iter::IntoParallelRefIterator;
pub use rayon_core::par_iter::IntoParallelRefMutIterator;
pub use rayon_core::par_iter::ParallelIterator;
pub use rayon_core::par_iter::BoundedParallelIterator;
pub use rayon_core::par_iter::ExactParallelIterator;
pub use rayon_core::par_iter::IndexedParallelIterator;

pub use rayon_core::par_iter::ToParallelChunks;
pub use rayon_core::par_iter::ToParallelChunksMut;

pub use rayon_core::par_iter::ParallelString;

pub use rayon_core::par_iter::FromParallelIterator;
