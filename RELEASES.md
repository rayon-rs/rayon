# Release rayon 0.7.1 / rayon-core 1.0.2

This release is a targeted performance fix for #343, an issue where
rayon threads could sometimes enter into a spin loop where they would
be unable to make progress until they are pre-empted.

# Release rayon 0.7 / rayon-core 1.0

This release marks the first step towards Rayon 1.0. **For best
performance, it is important that all Rayon users update to at least
Rayon 0.7.** This is because, as of Rayon 0.7, we have taken steps to
ensure that, no matter how many versions of rayon are actively in use,
there will only be a single global scheduler. This is achieved via the
`rayon-core` crate, which is being released at version 1.0, and which
encapsulates the core schedule APIs like `join()`. (Note: the
`rayon-core` crate is, to some degree, an implementation detail, and
not intended to be imported directly; it's entire API surface is
mirrored through the rayon crate.)

We have also done a lot of work reorganizing the API for Rayon 0.7 in
preparation for 1.0. The names of iterator types have been changed and
reorganized (but few users are expected to be naming those types
explicitly anyhow). In addition, a number of parallel iterator methods
have been adjusted to match those in the standard iterator traits more
closely. See the "Breaking Changes" section below for
details.

Finally, Rayon 0.7 includes a number of new features and new parallel
iterator methods. **As of this release, Rayon's parallel iterators
have officially reached parity with sequential iterators** -- that is,
every sequential iterator method that makes any sense in parallel is
supported in some capacity.

### New features and methods

- The internal `Producer` trait now features `fold_with`, which enables
  better performance for some parallel iterators.
- Strings now support `par_split()` and `par_split_whitespace()`.
- The `Configuration` API is expanded and simplified:
    - `num_threads(0)` no longer triggers an error 
    - you can now supply a closure to name the Rayon threads that get created 
      by using `Configuration::thread_name`.
    - you can now inject code when Rayon threads start up and finish
    - you can now set a custom panic handler to handle panics in various odd situations
- Threadpools are now able to more gracefully put threads to sleep when not needed.
- Parallel iterators now support `find_first()`, `find_last()`, `position_first()`,
  and `position_last()`.
- Parallel iterators now support `rev()`, which primarily affects subsequent calls
  to `enumerate()`.
- The `scope()` API is now considered stable (and part of `rayon-core`).
- There is now a useful `rayon::split` function for creating custom
  Rayon parallel iterators.
- Parallel iterators now allow you to customize the min/max number of
  items to be processed in a given thread. This mechanism replaces the
  older `weight` mechanism, which is deprecated.
- `sum()` and friends now use the standard `Sum` traits

### Breaking changes

In the move towards 1.0, there have been a number of minor breaking changes:

- Configuration setters like `Configuration::set_num_threads()` lost the `set_` prefix,
  and hence become something like `Configuration::num_threads()`.
- `Configuration` getters are removed
- Iterator types have been shuffled around and exposed more consistently:
    - combinator types live in `rayon::iter`, e.g. `rayon::iter::Filter`
    - iterators over various types live in a module named after their type,
      e.g. `rayon::slice::Windows`
- When doing a `sum()` or `product()`, type annotations are needed for the result
  since it is now possible to have the resulting sum be of a type other than the value
  you are iterating over (this mirrors sequential iterators).

### Experimental features

Experimental features require the use of the `unstable` feature. Their
APIs may change or disappear entirely in future releases (even minor
releases) and hence they should be avoided for production code.

- We now have (unstable) support for futures integration. You can use
  `Scope::spawn_future` or `rayon::spawn_future_async()`.
- There is now a `rayon::spawn_async()` function for using the Rayon
  threadpool to run tasks that do not have references to the stack.

### Contributors

Thanks to the following people for their contributions to this release:

- @Aaronepower
- @ChristopherDavenport
- @bluss
- @cuviper
- @froydnj
- @gaurikholkar
- @hniksic
- @leodasvacas
- @leshow
- @martinhath
- @mbrubeck
- @nikomatsakis
- @pegomes
- @schuster
- @torkleyy

# Release 0.6

This release includes a lot of progress towards the goal of parity
with the sequential iterator API, though there are still a few methods
that are not yet complete. If you'd like to help with that effort,
[check out the milestone](https://github.com/nikomatsakis/rayon/issues?q=is%3Aopen+is%3Aissue+milestone%3A%22Parity+with+the+%60Iterator%60+trait%22)
to see the remaining issues.

**Announcement:** @cuviper has been added as a collaborator to the
Rayon repository for all of his outstanding work on Rayon, which
includes both internal refactoring and helping to shape the public
API. Thanks @cuviper! Keep it up.

- We now support `collect()` and not just `collect_with()`.
  You can use `collect()` to build a number of collections,
  including vectors, maps, and sets. Moreover, when building a vector
  with `collect()`, you are no longer limited to exact parallel iterators.
  Thanks @nikomatsakis, @cuviper!
- We now support `skip()` and `take()` on parallel iterators.
  Thanks @martinhath!
- **Breaking change:** We now match the sequential APIs for `min()` and `max()`.
  We also support `min_by_key()` and `max_by_key()`. Thanks @tapeinosyne!
- **Breaking change:** The `mul()` method is now renamed to `product()`,
  to match sequential iterators. Thanks @jonathandturner!
- We now support parallel iterator over ranges on `u64` values. Thanks @cuviper!  
- We now offer a `par_chars()` method on strings for iterating over characters
  in parallel. Thanks @cuviper!
- We now have new demos: a traveling salesman problem solver as well as matrix
  multiplication. Thanks @nikomatsakis, @edre!
- We are now documenting our minimum rustc requirement (currently
  v1.12.0).  We will attempt to maintain compatibility with rustc
  stable v1.12.0 as long as it remains convenient, but if new features
  are stabilized or added that would be helpful to Rayon, or there are
  bug fixes that we need, we will bump to the most recent rustc. Thanks @cuviper!
- The `reduce()` functionality now has better inlining.
  Thanks @bluss!
- The `join()` function now has some documentation. Thanks @gsquire!
- The project source has now been fully run through rustfmt.
  Thanks @ChristopherDavenport!
- Exposed helper methods for accessing the current thread index.
  Thanks @bholley!

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
