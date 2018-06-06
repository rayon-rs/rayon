Note: This is an unstable fork made for use in rustc

# Rayon

[![Rayon crate](https://img.shields.io/crates/v/rayon.svg)](https://crates.io/crates/rayon)
[![Rayon documentation](https://docs.rs/rayon/badge.svg)](https://docs.rs/rayon)
[![Travis Status](https://travis-ci.org/rayon-rs/rayon.svg?branch=master)](https://travis-ci.org/rayon-rs/rayon)
[![Appveyor status](https://ci.appveyor.com/api/projects/status/wre5dkx08gayy8hc/branch/master?svg=true)](https://ci.appveyor.com/project/cuviper/rayon/branch/master)
[![Join the chat at https://gitter.im/rayon-rs/Lobby](https://badges.gitter.im/rayon-rs/Lobby.svg)](https://gitter.im/rayon-rs/Lobby?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

Rayon is a data-parallelism library for Rust. It is extremely
lightweight and makes it easy to convert a sequential computation into
a parallel one. It also guarantees data-race freedom. (You may also
enjoy [this blog post][blog] about Rayon, which gives more background
and details about how it works, or [this video][video], from the Rust
Belt Rust conference.) Rayon is
[available on crates.io](https://crates.io/crates/rayon), and
[API Documentation is available on docs.rs](https://docs.rs/rayon/).

[blog]: http://smallcultfollowing.com/babysteps/blog/2015/12/18/rayon-data-parallelism-in-rust/
[video]: https://www.youtube.com/watch?v=gof_OEv71Aw

## Parallel iterators and more

Rayon makes it drop-dead simple to convert sequential iterators into
parallel ones: usually, you just change your `foo.iter()` call into
`foo.par_iter()`, and Rayon does the rest:

```rust
use rayon::prelude::*;
fn sum_of_squares(input: &[i32]) -> i32 {
    input.par_iter() // <-- just change that!
         .map(|&i| i * i)
         .sum()
}
```

[Parallel iterators] take care of deciding how to divide your data
into tasks; it will dynamically adapt for maximum performance. If you
need more flexibility than that, Rayon also offers the [join] and
[scope] functions, which let you create parallel tasks on your own.
For even more control, you can create [custom threadpools] rather than
using Rayon's default, global threadpool.

[Parallel iterators]: https://docs.rs/rayon/*/rayon/iter/index.html
[join]: https://docs.rs/rayon/*/rayon/fn.join.html
[scope]: https://docs.rs/rayon/*/rayon/fn.scope.html
[custom threadpools]: https://docs.rs/rayon/*/rayon/struct.ThreadPool.html

## No data races

You may have heard that parallel execution can produce all kinds of
crazy bugs. Well, rest easy. Rayon's APIs all guarantee **data-race
freedom**, which generally rules out most parallel bugs (though not
all). In other words, **if your code compiles**, it typically does the
same thing it did before.

For the most, parallel iterators in particular are guaranteed to
produce the same results as their sequential counterparts. One caevat:
If your iterator has side effects (for example, sending methods to
other threads through a [Rust channel] or writing to disk), those side
effects may occur in a different order. Note also that, in some cases,
parallel iterators offer alternative versions of the sequential
iterator methods that can have higher performance.

[Rust channel]: https://doc.rust-lang.org/std/sync/mpsc/fn.channel.html

## Using Rayon

[Rayon is available on crates.io](https://crates.io/crates/rayon). The
recommended way to use it is to add a line into your Cargo.toml such
as:

```toml
[dependencies]
rayon = "1.0"
```

and then add the following to to your `lib.rs`:

```rust
extern crate rayon;
```

To use the Parallel Iterator APIs, a number of traits have to be in
scope. The easiest way to bring those things into scope is to use the
[Rayon prelude](https://docs.rs/rayon/*/rayon/prelude/index.html).  In
each module where you would like to use the parallel iterator APIs,
just add:

```rust
use rayon::prelude::*;
```

Rayon currently requires `rustc 1.13.0` or greater.

## Contribution

Rayon is an open source project! If you'd like to contribute to Rayon, check out [the list of "help wanted" issues](https://github.com/rayon-rs/rayon/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22). These are all (or should be) issues that are suitable for getting started, and they generally include a detailed set of instructions for what to do. Please ask questions if anything is unclear! Also, check out the [Guide to Development](https://github.com/rayon-rs/rayon/wiki/Guide-to-Development) page on the wiki. Note that all code submitted in PRs to Rayon is assumed to [be licensed under Rayon's dual MIT/Apache2 licensing](https://github.com/rayon-rs/rayon/blob/master/README.md#license).

## Quick demo

To see Rayon in action, check out the `rayon-demo` directory, which
includes a number of demos of code using Rayon. For example, run this
command to get a visualization of an nbody simulation. To see the
effect of using Rayon, press `s` to run sequentially and `p` to run in
parallel.

```
> cd rayon-demo
> cargo +nightly run --release -- nbody visualize
```

For more information on demos, try:

```
> cd rayon-demo
> cargo +nightly run --release -- --help
```

**Note:** While Rayon is usable as a library with the stable compiler, running demos or executing tests requires nightly Rust.

## Other questions?

See [the Rayon FAQ][faq].

[faq]: https://github.com/rayon-rs/rayon/blob/master/FAQ.md

## License

Rayon is distributed under the terms of both the MIT license and the
Apache License (Version 2.0). See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT) for details. Opening a pull requests is
assumed to signal agreement with these licensing terms.
