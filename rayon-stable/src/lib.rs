extern crate rayon_core;

pub mod par_iter;

pub use rayon_core::prelude;

pub use rayon_core::Configuration;
pub use rayon_core::InitError;
pub use rayon_core::dump_stats;
pub use rayon_core::initialize;
pub use rayon_core::ThreadPool;
pub use rayon_core::join;
pub use rayon_core::{scope, Scope};
