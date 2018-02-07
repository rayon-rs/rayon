#![doc(html_root_url = "https://docs.rs/rayon/0.9")]
#![deny(missing_debug_implementations)]
#![cfg_attr(test, feature(conservative_impl_trait))]
#![cfg_attr(test, feature(i128_type))]
#![deny(missing_docs)]

//! Data-parallelism library that is easy to convert sequential computations into parallel.
//!
//! Rayon is lightweight and convenient for application to existing code.  It guarantees
//! data-race free executions, and takes advantage of parallelism when sensible, based
//! on work-load at runtime.  There are two categories of rayon workloads: parallel
//! iterators and multi-branched recursion (`join` method).
//!
//! # Parallel Iterators
//!
//! Parallel iterators are formed using [`par_iter`], [`par_iter_mut`], and [`into_par_iter`]
//! functions to iterate by shared reference, mutable reference, or by value respectively.
//! These iterators are chained with computations that can take the
//! shape of `map` or `for_each` as an example.  This solves [embarrassingly]
//! parallel tasks that are completely independent of one another.
//!
//! [`par_iter`]: iter/trait.IntoParallelRefIterator.html#tymethod.par_iter
//! [`par_iter_mut`]: iter/trait.IntoParallelRefMutIterator.html#tymethod.par_iter_mut
//! [`into_par_iter`]: iter/trait.IntoParallelIterator.html#tymethod.into_par_iter
//! [embarrassingly]: https://en.wikipedia.org/wiki/Embarrassingly_parallel
//!
//! # Examples
//!
//! Here a string is encrypted using ROT13 leveraging parallelism.  Once all the
//! threads are complete, they are collected into a string.
//!
//! ```
//! extern crate rayon;
//! use rayon::prelude::*;
//! # fn main() {
//! let mut chars: Vec<char> = "A man, a plan, a canal - Panama!".chars().collect();
//! let encrypted: String = chars.into_par_iter().map(|c| {
//!        match c {
//!            'A' ... 'M' | 'a' ... 'm' => ((c as u8) + 13) as char,
//!            'N' ... 'Z' | 'n' ... 'z' => ((c as u8) - 13) as char,
//!            _ => c
//!        }
//!    }).collect();
//!    assert_eq!(encrypted, "N zna, n cyna, n pnany - Cnanzn!");
//! # }
//! ```
//!
//! # Divide and conquer with `join`
//!
//! [`join`] takes two closures and runs them in parallel if doing so will improve
//! execution time.  Parallel Iterators are implemented using [`join`] with
//! work-stealing.  Given two tasks that safely run in parallel, one task is queued
//! and another starts processing.  If idle threads exist, they begin execution on
//! the queued work.
//!
//! [`join`]: fn.join.html
//!
//! # Examples
//!
//! ```rust,ignore
//! join(|| do_something(), || do_something_else())
//! ```
//!
//! ```rust
//! fn quick_sort<T:PartialOrd+Send>(v: &mut [T]) {
//!    if v.len() > 1 {
//!        let mid = partition(v);
//!        let (lo, hi) = v.split_at_mut(mid);
//!        rayon::join(|| quick_sort(lo),
//!                    || quick_sort(hi));
//!    }
//! }
//! # fn main() { }
//! # fn partition<T:PartialOrd+Send>(v: &mut [T]) -> usize { 0 }
//! ```
//!
//! # Rayon Types
//!
//! Rayon traits are bundled into [`rayon::prelude::*`].  To get access to parallel
//! implementations on various standard types include `use rayon::prelude::*;`
//!
//! These implementations will give you access to `par_iter` with parallel
//! implementations of iterative functions including [`map`], [`for_each`], [`filter`],
//! [`fold`], and [more].
//!
//! [`rayon::prelude::*`]: prelude/index.html
//! [`map`]: iter/trait.ParallelIterator.html#method.map
//! [`for_each`]: iter/trait.ParallelIterator.html#method.for_each
//! [`filter`]: iter/trait.ParallelIterator.html#method.filter
//! [`fold`]: iter/trait.ParallelIterator.html#method.fold
//! [more]: iter/trait.ParallelIterator.html#provided-methods
//!
//! # Crate Layout
//!
//! Rayon extends many of the types found in the standard library with
//! parallel iterator implementations. The modules in the `rayon`
//! crate mirror [`std`] itself: so, e.g., the `option` module in
//! Rayon contains parallel iterators for the `Option` type, which is
//! found in [the `option` module of `std`]. Similarly, the
//! `collections` module in Rayon offers parallel iterator types for
//! [the `collections` from `std`]. You will rarely need to access
//! these submodules unless you need to name iterator types
//! explicitly.
//!
//! [the `option` module of `std`]: https://doc.rust-lang.org/std/option/index.html
//! [the `collections` from `std`]: https://doc.rust-lang.org/std/collections/index.html
//! [`std`]: https://doc.rust-lang.org/std/

extern crate rayon_core;
extern crate either;

#[cfg(test)]
extern crate rand;

#[macro_use]
mod delegate;

#[macro_use]
mod private;

mod split_producer;

pub mod collections;
pub mod iter;
pub mod option;
pub mod prelude;
pub mod range;
pub mod result;
pub mod slice;
pub mod str;
pub mod vec;

mod par_either;
mod math;
mod test;

pub use rayon_core::current_num_threads;
pub use rayon_core::ThreadPool;
pub use rayon_core::ThreadPoolBuilder;
pub use rayon_core::{join, join_context};
pub use rayon_core::FnContext;
pub use rayon_core::{scope, Scope};
pub use rayon_core::spawn;
