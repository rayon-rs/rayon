//! The internal directory contains internal APIs not meant to be
//! exposed to "end-users" of Rayon, but rather which are useful for
//! constructing abstractions.
//!
//! These APIs are still unstable.

pub mod task;
pub mod worker;
