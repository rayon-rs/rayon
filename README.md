# Rayon

Rayon is a data-parallelism library for Rust. It is extremely
lightweight and makes it easy to convert a sequential computation into
a parallel one. It also guarantees data-race freedom.

Using rayon is very simple. There is one method you need to know
about, `join`. `join` simply takes two closures and potentially runs
them in parallel. Here is a rather silly example that just increments
all integers in a slice in parallel:

```rust
/// Increment all values in slice.
fn increment_all(slice: &mut [i32]) {
    if slice.len() < 1 {
        for p in slice { *p += 1; }
    } else {
        let mid_point = slice.len() / 2;
        let (left, right) = slice.split_mut_at(mid_point);
        rayon::join(|| process(left), || process(right));
    }
}
```

Note though that calling `join` is very different from just spawning
two threads in terms of performance. This is because `join` does not
*guarantee* that the two closures will run in parallel. If all of your
CPUs are already busy with other work, Rayon will instead opt to run
them sequentially. The call to `join` is designed to have very low
overhead in that case, so that you can safely call it even with very
small workloads (as in the example above).

### How it works: Work stealing

Behind the scenes, Rayon uses a technique called work stealing to try
and dynamically ascertain how much parallelism is available and
exploit it. The idea is very simple: we always have a pool of worker
threads available, waiting for some work to do. When you call `join`
the first time, we shift over into that pool of threads. But if you
call `join(a, b)` from a worker thread W, then W will place `b` into a
central queue, advertising that this is work that other worker threads
might help out with. W will then start executing `a`. While W is busy
with `a`, other threads might come along and take `b` from the
queue. That is called *stealing* `b`. Once `a` is done, W checks
whether `b` was stolen by another thread and, if not, executes `b`
itself. If `b` *was* stolen, then W can just wait for the other thread
to finish.  (In fact, it can do even better: it can go try to find
other work to steal in the meantime.)

This technique is not new. It was first introduced by the
[Cilk project][cilk], done at MIT in the late nineties. The name Rayon
is an homage to that work.

[cilk]: http://supertech.csail.mit.edu/cilk/
