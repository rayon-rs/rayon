# Release 0.5

- **Breaking change:** The `reduce` method has been vastly
  simplified, and `reduce_with_identity` has been deprecated.
- **Breaking change:** The `fold` method has been changed. It used to
  always reduce the values, but now instead it is a combinator that
  returns a parallel iterator which can itself be reduced. See the
  docs for more information.
- The following parallel iterator combinators are now available (thanks @cuviper!):
  - `find_any()`: similar to `find` on a sequential iterator,
    but doesn't necessarily return the *first* matching item
  - `position_any()`: similar to `position` on a sequential iterator,
    but doesn't necessarily return the index of *first* matching item
  - `any()`, `all()`: just like their sequential counterparts
- The `count()` combinator is now available for parallel iterators.
- We now build with older versions of rustc again (thanks @durango!),
  as we removed a stray semicolon from `thread_local!`.
- Various improvements to the (unstable) `scope()` API implementation.
    
# Release 0.4.3

- Parallel iterators now offer an adaptive weight scheme,
  which means that explicit weights should no longer
  be necessary in most cases! Thanks @cuviper!
  - We are considering removing weights or changing the weight mechanism
    before 1.0. Examples of scenarios where you still need weights even
    with this adaptive mechanism would be great. Join the discussion
    at <https://github.com/nikomatsakis/rayon/issues/111>.
- New (unstable) scoped threads API, see `rayon::scope` for details.
  - You will need to supply the [cargo feature] `unstable`.
- The various demos and benchmarks have been consolidated into one
  program, `rayon-demo`.
- Optimizations in Rayon's inner workings. Thanks @emilio!  
- Update `num_cpus` to 1.0. Thanks @jamwt!
- Various internal cleanup in the implementation and typo fixes.
  Thanks @cuviper, @Eh2406, and @spacejam!

[cargo feature]: http://doc.crates.io/manifest.html#the-features-section

# Release 0.4.2

- Updated crates.io metadata.

# Release 0.4.1

- New `chain` combinator for parallel iterators.
- `Option`, `Result`, as well as many more collection types now have
  parallel iterators.
- New mergesort demo.
- Misc fixes.

Thanks to @cuviper, @edre, @jdanford, @frewsxcv for their contributions!

# Release 0.4

- Make use of latest versions of catch-panic and various fixes to panic propagation.
- Add new prime sieve demo.
- Add `cloned()` and `inspect()` combinators.
- Misc fixes for Rust RFC 1214.

Thanks to @areilb1, @Amanieu, @SharplEr, and @cuviper for their contributions!

# Release 0.3

- Expanded `par_iter` APIs now available:
  - `into_par_iter` is now supported on vectors (taking ownership of the elements)
- Panic handling is much improved:
  - if you use the Nightly feature, experimental panic recovery is available
  - otherwise, panics propagate out and poision the workpool
- New `Configuration` object to control number of threads and other details
- New demos and benchmarks
  - try `cargo run --release -- visualize` in `demo/nbody` :)
    - Note: a nightly compiler is required for this demo due to the
      use of the `+=` syntax

Thanks to @bjz, @cuviper, @Amanieu, and @willi-kappler for their contributions!

# Release 0.2 and earlier

No release notes were being kept at this time.
