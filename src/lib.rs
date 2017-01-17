#![allow(non_camel_case_types)] // I prefer to use ALL_CAPS for type parameters
#![cfg_attr(test, feature(conservative_impl_trait))]

extern crate deque;
#[macro_use]
extern crate lazy_static;
#[cfg(feature = "unstable")]
extern crate futures;
extern crate libc;
extern crate num_cpus;
extern crate rand;

#[macro_use]
mod log;

mod configuration;
mod latch;
mod join;
mod job;
pub mod par_iter;
pub mod prelude;
#[cfg(test)]
mod test;
mod registry;
mod future;
mod scope;
mod sleep;
mod spawn_async;
mod thread_pool;
mod unwind;
mod util;

pub use configuration::Configuration;
pub use configuration::PanicHandler;
pub use configuration::InitError;
pub use configuration::dump_stats;
pub use configuration::initialize;
pub use thread_pool::ThreadPool;
pub use join::join;
pub use scope::{scope, Scope};
pub use spawn_async::spawn_async;
pub use future::RayonFuture;
