# Rayon

[![Join the chat at https://gitter.im/rayon-rs/Lobby](https://badges.gitter.im/rayon-rs/Lobby.svg)](https://gitter.im/rayon-rs/Lobby?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

[![Travis Status](https://travis-ci.org/nikomatsakis/rayon.svg?branch=master)](https://travis-ci.org/nikomatsakis/rayon)

[![Appveyor status](https://ci.appveyor.com/api/projects/status/6oft3iwgr6f2o4d4?svg=true)](https://ci.appveyor.com/project/nikomatsakis/rayon)

Rayon is a data-parallelism library for Rust. It is extremely
lightweight and makes it easy to convert a sequential computation into
a parallel one. It also guarantees data-race freedom. (You may also
enjoy [this blog post][blog] about Rayon, which gives more background
and details about how it works, or [this video][video], from the Rust Belt Rust conference.) Rayon is
[available on crates.io](https://crates.io/crates/rayon), and
[API Documentation is available on docs.rs](https://docs.rs/rayon/).

[blog]: http://smallcultfollowing.com/babysteps/blog/2015/12/18/rayon-data-parallelism-in-rust/
[video]: https://www.youtube.com/watch?v=gof_OEv71Aw

You can use Rayon in two ways. Which way you will want will depend on
what you are doing:

- Parallel iterators: convert iterator chains to execute in parallel.
- The `join` method: convert recursive, divide-and-conquer style
  problems to execute in parallel.

No matter which way you choose, you don't have to worry about data
races: Rayon statically guarantees data-race freedom. For the most
part, adding calls to Rayon should not change how your programs works
at all, in fact. However, if you operate on mutexes or atomic
integers, please see the [notes on atomicity](#atomicity).

Rayon currently requires `rustc 1.12.0` or greater.

### Using Rayon

[Rayon is available on crates.io](https://crates.io/crates/rayon). The
recommended way to use it is to add a line into your Cargo.toml such
as:

```rust
[dependencies]
rayon = 0.8.1
```

and then add the following to to your `lib.rs`:

```rust
extern crate rayon;
```

To use the Parallel Iterator APIs, a number of traits have to be in
scope. The easiest way to bring those things into scope is to use the
[Rayon prelude](https://docs.rs/rayon/*/rayon/prelude/index.html).
In each module where you would like to use the parallel iterator APIs,
just add:

```rust
use rayon::prelude::*;
```

### Contribution

Rayon is an open source project! If you'd like to contribute to Rayon, check out [the list of "help wanted" issues](https://github.com/nikomatsakis/rayon/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22). These are all (or should be) issues that are suitable for getting started, and they generally include a detailed set of instructions for what to do. Please ask questions if anything is unclear! Also, check out the [Guide to Development](https://github.com/nikomatsakis/rayon/wiki/Guide-to-Development) page on the wiki. Note that all code submitted in PRs to Rayon is assumed to [be licensed under Rayon's dual MIT/Apache2 licensing](https://github.com/nikomatsakis/rayon/blob/master/README.md#license).

### Quick demo

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

### Parallel Iterators

Rayon supports an experimental API called "parallel iterators". These
let you write iterator-like chains that execute in parallel. For
example, to compute the sum of the squares of a sequence of integers,
one might write:

```rust
use rayon::prelude::*;
fn sum_of_squares(input: &[i32]) -> i32 {
    input.par_iter()
         .map(|&i| i * i)
         .sum()
}
```

Or, to increment all the integers in a slice, you could write:

```rust
use rayon::prelude::*;
fn increment_all(input: &mut [i32]) {
    input.par_iter_mut()
         .for_each(|p| *p += 1);
}
```

To use parallel iterators, first import the traits by adding something
like `use rayon::prelude::*` to your module. You can then call
`par_iter` and `par_iter_mut` to get a parallel iterator.  Like a
[regular iterator][], parallel iterators work by first constructing a
computation and then executing it. See the
[`ParallelIterator` trait][pt] for the list of available methods and
more details. (Sorry, proper documentation is still somewhat lacking.)

[regular iterator]: http://doc.rust-lang.org/std/iter/trait.Iterator.html
[pt]: https://github.com/nikomatsakis/rayon/blob/master/src/iter/mod.rs

### Using join for recursive, divide-and-conquer problems

Parallel iterators are actually implemented in terms of a more
primitive method called `join`. `join` simply takes two closures and
potentially runs them in parallel. For example, we could rewrite the
`increment_all` function we saw for parallel iterators as follows
(this function increments all the integers in a slice):

```rust
/// Increment all values in slice.
fn increment_all(slice: &mut [i32]) {
    if slice.len() < 1000 {
        for p in slice { *p += 1; }
    } else {
        let mid_point = slice.len() / 2;
        let (left, right) = slice.split_at_mut(mid_point);
        rayon::join(|| increment_all(left), || increment_all(right));
    }
}
```

Perhaps a more interesting example is this parallel quicksort:

```rust
fn quick_sort<T:PartialOrd+Send>(v: &mut [T]) {
    if v.len() <= 1 {
        return;
    }

    let mid = partition(v);
    let (lo, hi) = v.split_at_mut(mid);
    rayon::join(|| quick_sort(lo), || quick_sort(hi));
}
```

**Note though that calling `join` is very different from just spawning
two threads in terms of performance.** This is because `join` does not
*guarantee* that the two closures will run in parallel. If all of your
CPUs are already busy with other work, Rayon will instead opt to run
them sequentially. The call to `join` is designed to have very low
overhead in that case, so that you can safely call it even with very
small workloads (as in the example above).

However, in practice, the overhead is still noticeable. Therefore, for
maximal performance, you want to have some kind of sequential fallback
once your problem gets small enough. The parallel iterator APIs try to
handle this for you. When using join, you have to code it yourself.
For an example, see the [quicksort demo][], which includes sequential
fallback after a certain size.

[quicksort demo]: https://github.com/nikomatsakis/rayon/blob/master/rayon-demo/src/quicksort/mod.rs

### Safety

You've probably heard that parallel programming can be the source of
bugs that are really hard to diagnose. That is certainly true!
However, thanks to Rust's type system, you basically don't have to
worry about that when using Rayon. The Rayon APIs are guaranteed to be
data-race free. The Rayon APIs themselves also cannot cause deadlocks
(though if your closures or callbacks use locks or ports, those locks
might trigger deadlocks).

For example, if you write code that tries to process the same mutable
state from both closures, you will find that fails to compile:

```rust
/// Increment all values in slice.
fn increment_all(slice: &mut [i32]) {
    rayon::join(|| process(slice), || process(slice));
}
```

However, this safety does have some implications. You will not be able
to use types which are not thread-safe (i.e., do not implement `Send`)
from inside a `join` closure. Note that almost all types *are* in fact
thread-safe in Rust; the only exception is those types that employ
"inherent mutability" without some form of synchronization, such as
`RefCell` or `Rc`. Here is a list of the most common types in the
standard library that are not `Send`, along with an alternative that
you can use instead which *is* `Send` (but which also has higher
overhead, because it must work across threads):

- `Cell` -- replacement: `AtomicUsize`, `AtomicBool`, etc (but see warning below)
- `RefCell` -- replacement: `RwLock`, or perhaps `Mutex` (but see warning below)
- `Rc` -- replacement: `Arc`

However, if you are converting uses of `Cell` or `RefCell`, you must
be prepared for other threads to interject changes. For more
information, read the section on atomicity below.

### How it works: Work stealing

Behind the scenes, Rayon uses a technique called work stealing to try
and dynamically ascertain how much parallelism is available and
exploit it. The idea is very simple: we always have a pool of worker
threads available, waiting for some work to do. When you call `join`
the first time, we shift over into that pool of threads. But if you
call `join(a, b)` from a worker thread W, then W will place `b` into
its work queue, advertising that this is work that other worker
threads might help out with. W will then start executing `a`.

While W is busy with `a`, other threads might come along and take `b`
from its queue. That is called *stealing* `b`. Once `a` is done, W
checks whether `b` was stolen by another thread and, if not, executes
`b` itself. If W runs out of jobs in its own queue, it will look
through the other threads' queues and try to steal work from them.

This technique is not new. It was first introduced by the
[Cilk project][cilk], done at MIT in the late nineties. The name Rayon
is an homage to that work.

[cilk]: http://supertech.csail.mit.edu/cilk/

<a name="atomicity"></a>

#### Warning: Be wary of atomicity

Converting a `Cell` (or, to a lesser extent, a `RefCell`) to work in
parallel merits special mention for a number of reasons. `Cell` and
`RefCell` are handy types that permit you to modify data even when
that data is shared (aliased). They work somewhat differently, but
serve a common purpose:

1. A `Cell` offers a mutable slot with just two methods, `get` and
   `set`.  Cells can only be used for `Copy` types that are safe to
   memcpy around, such as `i32`, `f32`, or even something bigger like a tuple of
   `(usize, usize, f32)`.
2. A `RefCell` is kind of like a "single-threaded read-write lock"; it
   can be used with any sort of type `T`. To gain access to the data
   inside, you call `borrow` or `borrow_mut`. Dynamic checks are done
   to ensure that you have either readers or one writer but not both.

While there are threadsafe types that offer similar APIs, caution is
warranted because, in a threadsafe setting, other threads may
"interject" modifications in ways that are not possible in sequential
code. While this will never lead to a *data race* --- that is, you
need not fear *undefined behavior* --- you can certainly still have
*bugs*.

Let me give you a concrete example using `Cell`. A common use of `Cell`
is to implement a shared counter. In that case, you would have something
like `counter: Rc<Cell<usize>>`. Now I can increment the counter by
calling `get` and `set` as follows:

```rust
let value = counter.get();
counter.set(value + 1);
```

If I convert this to be a thread-safe counter, I would use the
corresponding types `tscounter: Arc<AtomicUsize>`. If I then were to
convert the `Cell` API calls directly, I would do something like this:

```rust
let value = tscounter.load(Ordering::SeqCst);
tscounter.store(value + 1, Ordering::SeqCst);
```

You can already see that the `AtomicUsize` API is a bit more complex,
as it requires you to specify an
[ordering](http://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html). (I
won't go into the details on ordering here, but suffice to say that if
you don't know what an ordering is, and probably even if you do, you
should use `Ordering::SeqCst`.) The danger in this parallel version of
the counter is that other threads might be running at the same time
and they could cause our counter to get out of sync. For example, if
we have two threads, then they might both execute the "load" before
either has a chance to execute the "store":

```
Thread 1                                          Thread 2
let value = tscounter.load(Ordering::SeqCst);
// value = X                                      let value = tscounter.load(Ordering::SeqCst);
                                                  // value = X
tscounter.store(value+1);                         tscounter.store(value+1);
// tscounter = X+1                                // tscounter = X+1
```

Now even though we've had two increments, we'll only increase the
counter by one!  Even though we've got no data race, this is still
probably not the result we wanted. The problem here is that the `Cell`
API doesn't make clear the scope of a "transaction" -- that is, the
set of reads/writes that should occur atomically. In this case, we
probably wanted the get/set to occur together.

In fact, when using the `Atomic` types, you very rarely want a plain
`load` or plain `store`. You probably want the more complex
operations. A counter, for example, would use `fetch_add` to
atomically load and increment the value in one step. Compare-and-swap
is another popular building block.

A similar problem can arise when converting `RefCell` to `RwLock`, but
it is somewhat less likely, because the `RefCell` API does in fact
have a notion of a transaction: the scope of the handle returned by
`borrow` or `borrow_mut`. So if you convert each call to `borrow` to
`read` (and `borrow_mut` to `write`), things will mostly work fine in
a parallel setting, but there can still be changes in behavior.
Consider using a `handle: RefCell<Vec<i32>>` like :

```rust
let len = handle.borrow().len();
for i in 0 .. len {
    let data = handle.borrow()[i];
    println!("{}", data);
}
```

In sequential code, we know that this loop is safe. But if we convert
this to parallel code with an `RwLock`, we do not: this is because
another thread could come along and do
`handle.write().unwrap().pop()`, and thus change the length of the
vector. In fact, even in *sequential* code, using very small borrow
sections like this is an anti-pattern: you ought to be enclosing the
entire transaction together, like so:

```rust
let vec = handle.borrow();
let len = vec.len();
for i in 0 .. len {
    let data = vec[i];
    println!("{}", data);
}
```

Or, even better, using an iterator instead of indexing:

```rust
let vec = handle.borrow();
for data in vec {
    println!("{}", data);
}
```

There are several reasons to prefer one borrow over many. The most
obvious is that it is more efficient, since each borrow has to perform
some safety checks. But it's also more reliable: suppose we modified
the loop above to not just print things out, but also call into a
helper function:

```rust
let vec = handle.borrow();
for data in vec {
    helper(...);
}
```

And now suppose, independently, this helper fn evolved and had to pop
something off of the vector:

```rust
fn helper(...) {
    handle.borrow_mut().pop();
}
```

Under the old model, where we did lots of small borrows, this would
yield precisely the same error that we saw in parallel land using an
`RwLock`: the length would be out of sync and our indexing would fail
(note that in neither case would there be an actual *data race* and
hence there would never be undefined behavior). But now that we use a
single borrow, we'll see a borrow error instead, which is much easier
to diagnose, since it occurs at the point of the `borrow_mut`, rather
than downstream. Similarly, if we move to an `RwLock`, we'll find that
the code either deadlocks (if the write is on the same thread as the
read) or, if the write is on another thread, works just fine. Both of
these are preferable to random failures in my experience.

#### But wait, isn't Rust supposed to free me from this kind of thinking?

You might think that Rust is supposed to mean that you don't have to
think about atomicity at all. In fact, if you avoid inherent
mutability (`Cell` and `RefCell` in a sequential setting, or
`AtomicUsize`, `RwLock`, `Mutex`, et al. in parallel code), then this
is true: the type system will basically guarantee that you don't have
to think about atomicity at all. But often there are times when you
WANT threads to interleave in the ways I showed above.

Consider for example when you are conducting a search in parallel, say
to find the shortest route. To avoid fruitless search, you might want
to keep a cell with the shortest route you've found thus far.  This
way, when you are searching down some path that's already longer than
this shortest route, you can just stop and avoid wasted effort. In
sequential land, you might model this "best result" as a shared value
like `Rc<Cell<usize>>` (here the `usize` represents the length of best
path found so far); in parallel land, you'd use a `Arc<AtomicUsize>`.
Now we can make our search function look like:

```rust
fn search(path: &Path, cost_so_far: usize, best_cost: &Arc<AtomicUsize>) {
    if cost_so_far >= best_cost.load(Ordering::SeqCst) {
        return;
    }
    ...
    best_cost.store(...);
}
```

Now in this case, we really WANT to see results from other threads
interjected into our execution!

## Semver policy, the rayon-core crate, and unstable features

Rayon follows semver versioning. However, we also have APIs that are
still in the process of development and which may break from release
to release -- those APIs are not subject to semver. To use them,
you have to set the cfg flag `rayon_unstable`. The easiest way to do this
is to use the `RUSTFLAGS` environment variable:

```
RUSTFLAGS='--cfg rayon_unstable' cargo build
```

Note that this must not only be done for your crate, but for any crate
that depends on your crate. This infectious nature is intentional, as
it serves as a reminder that you are outside of the normal semver
guarantees. **If you see unstable APIs that you would like to use,
please request stabilization on the correspond tracking issue!**

Rayon itself is internally split into two crates. The `rayon` crate is
intended to be the main, user-facing crate, and hence all the
documentation refers to `rayon`. This crate is still evolving and
regularly goes through (minor) breaking changes. The `rayon-core`
crate contains the global thread-pool and defines the core APIs: we no
longer permit breaking changes in this crate (except to unstable
features). The intention is that multiple semver-incompatible versions
of the rayon crate can peacefully coexist; they will all share one
global thread-pool through the `rayon-core` crate.

## License

Rayon is distributed under the terms of both the MIT license and the
Apache License (Version 2.0). See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT) for details. Opening a pull requests is
assumed to signal agreement with these licensing terms.
