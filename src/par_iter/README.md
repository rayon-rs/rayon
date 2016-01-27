# Parallel Iterators

These are some notes on the design of the parallel iterator traits.
This file does not describe how to **use** parallel iterators.

## The challenge

Parallel iterators are more complicated than sequential iterators.
The reason is that they have to be able to split themselves up and
operate in parallel across the two halves.

The current design for parallel iterators has two distinct modes in
which they can be used; as we will see, not all iterators support both
modes (which is why there are two):

- **Pull mode** (the `Producer` trait): in this mode, the iterator is
  asked to produce the next item using a call to `next`. This is
  basically like a normal iterator, but with a twist: you can split
  the iterator at a given point by calling `split_at`, which produces
  two iterators.
- **Push mode** (the `Consumer` and `UnindexedConsumer` traits): in
  this mode, the iterator instead is *given* each item in turn, which
  is then processed. This is the opposite of a normal iterator. It's
  more like a `for_each` call: each time a new item is produced, the
  `consume` method is called with that item. (The traits themselves are
  a bit more complex, as they support state that can be threaded
  through and ultimately reduced.) Unlike producers, there are two
  variants of consumers. The difference is how the split is performed:
  - in the `Consumer` trait, splitting is done with `split_at`, which
    accepts an index where the split should be performed. All
    iterators can work in this mode. The resulting halves thus have an
    idea about how much data they expect to consume.
  - in the `UnindexedConsumer` trait, splitting is done with `split`.
    There is no index: the resulting halves must be prepared to
    process any amount of data, and they don't know where that data
    falls in the overall stream.
    - Not all consumers can operate in this mode. It works for
      `for_each` and `reduce`, for example, but it does not work for
      `collect_into`, since in that case the position of each item is
      important for knowing where it ends up in the target collection.

## How iterator execution proceeds  

We'll walk through this example iterator chain to start. This chain
demonstrates more-or-less the full complexity of what can happen.

```rust
vec1.par_iter()
    .zip(vec2.par_iter())
    .flat_map(some_function)
    .for_each(some_other_function)
```

To handle an iterator chain, we start by creating consumers. This
works from the end. So in this case, the call to `for_each` is the
final step, so it will create a `ForEachConsumer` that, given an item,
just calls `some_other_function` with that item. (`ForEachConsumer` is
a very simple consumer because it doesn't need to thread any state
between items at all.)

Now, the `for_each` call will pass this consumer to the base iterator,
which is the `flat_map`. It will do by calling the `drive_unindexed`
method on the `ParallelIterator` trait. `drive_unindexed` basically
says "produce items for this iterator and feed them to this consumer";
it only works for unindexed consumers.

(As an aside, it is interesting that only some consumers can work in
unindexed mode, but all producers can *drive* an unindexed consumer.
In contrast, only some producers can drive an *indexed* consumer, but
all consumers can be supplied indexes. Isn't variance neat.)

As it happens, `FlatMap` only works with unindexed consumers anyway.
This is because flat-map basically has no idea how many items it will
produce. If you ask flat-map to produce the 22nd item, it can't do it,
at least not without some intermediate state. It doesn't know whether
processing the first item will create 1 item, 3 items, or 100;
therefore, to produce an arbitrary item, it would basically just have
to start at the beginning and execute sequentially, which is not what
we want. But for unindexed consumers, this doesn't matter, since they
don't need to know how much data they will get.

Therefore, `FlatMap` can wrap the `ForEachConsumer` with a
`FlatMapConsumer` that feeds to it. This `FlatMapConsumer` will be
given one item. It will then invoke `some_function` to get a parallel
iterator out. It will then ask this new parallel iterator to drive the
`ForEachConsumer`. The `drive_unindexed` method on `flat_map` can then
pass the `FlatMapConsumer` up the chain to the previous item, which is
`zip`. At this point, something interesting happens.

## Switching from push to pull mode

If you think about `zip`, it can't really be implemented as a
consumer, at least not without an intermediate thread and some
channels or something (or maybe coroutines). The problem is that it
has to walk two iterators *in lockstep*. Basically, it can't call two
`drive` methods simultaneously, it can only call one at a time. So at
this point, the `zip` iterator needs to switch from *push mode* into
*pull mode*.

You'll note that `Zip` is only usable if its inputs implement
`IndexedParallelIterator`, meaning that they can produce data starting
at random points in the stream. This need to switch to push mode is
exactly why. If we want to split a zip iterator at position 22, we
need to be able to start zipping items from index 22 right away,
without having to start from index 0.

Anyway, so at this point, the `drive_unindexed` method for `Zip` stops
creating consumers. Instead, it creates a *producer*, a `ZipProducer`,
to be exact, and calls the `bridge` function in the `internals`
module. Creating a `ZipProducer` will in turn create producers for
the two iterators being zipped. This is possible because they both
implement `IndexedParallelIterator`.

The `bridge` function will then connect the consumer, which is
handling the `flat_map` and `for_each`, with the producer, which is
handling the `zip` and its preecessors. It will split down until the
chunks seem reasonably small, then pull items from the producer and
feed them to the consumer.

## The base case

The other time that `bridge` gets used is when we bottom out in an
indexed producer, such as a slice or range.

## Why is there no `UnindexedProducer`?

You may be wondering why there is no `UnindexedProducer` trait.  The
answer is that there isn't really a need for one: the
`drive_unindexed` method basically *is* an unindexed producer. As it
happens, in the current code base, all parallel iterators bottom out
in something indexed (a slice, a range, etc), but we may extend that
in the future, no problem.
