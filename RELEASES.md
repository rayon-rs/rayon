# Release 0.4.3 (pending)

- New (unstable) scoped threads API.

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
