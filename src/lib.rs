#![allow(non_camel_case_types)] // I prefer to use ALL_CAPS for type parameters
#![cfg_attr(test, feature(conservative_impl_trait))]

// If you're not compiling the unstable code, it often happens that
// there is stuff that is considered "dead code" and so forth. So
// disable warnings in that scenario.
#![cfg_attr(not(feature = "unstable"), allow(warnings))]

extern crate rayon_core;

#[cfg(test)]
extern crate rand;

#[macro_use]
mod delegate;

#[macro_use]
mod private;

pub mod collections;
pub mod iter;
pub mod option;
pub mod prelude;
pub mod range;
pub mod result;
pub mod slice;
pub mod str;
pub mod vec;

mod test;

pub use iter::split;

pub use rayon_core::current_num_threads;
pub use rayon_core::Configuration;
pub use rayon_core::initialize;
pub use rayon_core::ThreadPool;
pub use rayon_core::join;
pub use rayon_core::{scope, Scope};
#[cfg(feature = "unstable")]
pub use rayon_core::spawn_async;
#[cfg(feature = "unstable")]
pub use rayon_core::spawn_future_async;
#[cfg(feature = "unstable")]
pub use rayon_core::RayonFuture;
