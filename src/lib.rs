#![allow(non_camel_case_types)] // I prefer to use ALL_CAPS for type parameters

extern crate deque;
extern crate num_cpus;
extern crate rand;

#[macro_use]
mod log;

mod api;
mod latch;
mod job;
pub mod par_iter;
#[cfg(test)] mod test;
mod thread_pool;
mod util;

pub use api::Configuration;
pub use api::InitResult;
pub use api::dump_stats;
pub use api::join;
pub use api::ThreadPool;
