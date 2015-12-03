#![allow(non_camel_case_types)] // I prefer to use ALL_CAPS for type parameters

extern crate rand;

mod api;
mod latch;
mod job;
#[cfg(test)] mod test;
mod thread_pool;
mod util;

pub use api::initialize;
pub use api::join;
