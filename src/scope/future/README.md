# Future integration into Rayon

## How futures work

Let's start with a brief coverage of how futures work. Our example will
be a simple chain of futures:

    F_map -> F_socket

Here `F_socket` is a future that maps to a TCP socket. It returns a
`Vec<u8>` of data read from that socket. `F_map` is a future will take
that data and do some transformation. (Note that the real futures for
reading from sockets etc do not work in this way, this is just an
example.)

The idea of futures is that each future offers a `poll()` method. When
`poll()` is invoked, the future will attempt to execute. Typically,
this often involves recursively calling `poll()` on other futures. So,
in our example, `F_map` when it starts would call `F_socket.poll()` to
see if the data is ready. The idea is that `poll()` returns one of
three values:

- `Ok(Async::Ready(R))` -- the future has completed, here is the result `R`.
- `Err(E)` -- the future has completed and resulted in an error `E`.
- `Ok(Async::NotReady)` -- the future is not yet complete.

The last one is the most interesting. It means that the future is
blocked on *some event X*, typically an I/O event (i.e., we are
waiting for more data to arrive on a TCP socket).

When a future returns `NotReady`, it also has one additional job. It
must register the "current task" (think for now of the current thread)
to be re-awoken when the event X has occurred. For most futures, this
job is delegated to another future: e.g., in our example, `F_map`
invokes `F_socket.poll()`. So if `F_socket.poll()` returns not-ready,
then it will have registered the current thread already, and hence
`F_map` can merely propagates the `NotReady` result further up.

### The current task and executor

A key concept of the futures.rs library is that of an *executor*.  The
executor is the runtime that first invokes the top-level future
(`T_map`, in our example). This is precisely the role that Rayon
plays. Note that in any futures system there may be many
interoperating execturs though.

Part of an executors job is to maintain some thread-local storage
(TLS) when a future is executing. In particular, it must setup the
"current task" (basically a unique integer, although it's an opaque
type) as well as an "unpark object" of type
`Arc<Unpark>`. [The `Unpark` trait][unpark] offers a single method
(`unpark()`) which can be invoked when the task should be
re-awoken. So `F_socket` might, for example, get the current
`Arc<Unpark>` object and store it for use by an I/O thread. The I/O
thread might invoke `epoll()` or `select()` or whatever and, when it
detects the socket has more data, invoke the `unpark()` method.

[unpark]: https://docs.rs/futures/0.1/futures/executor/trait.Unpark.html

## Rayon's futures integration

When you spawn a future of type `F` into rayon, the idea is that it is
going to start independently executing in the thread-pool. Meanwhile,
the `spawn_future()` method returns to you your own future (let's call
it `F'`) that you can use to poll and monitor its progress. Internally
within Rayon, however, we only allocate a single `Arc` to represent
both of these things -- an `Arc<ScopeFuture<F>>`, to be precise -- and
this `Arc` hence serves two distinct roles.

The operations on `F'` (the handle returned to the user) are specified
by the trait `ScopeFutureTrait` and are very simple. The user can
either `poll()` the future, which is checking to see if rayon is done
executing it yet, or `cancel()` the future. `cancel()` occurs when
`F'` is dropped, which indicates that the user no longer has interest
in the result.

### Future reference counting

Each spawned future is represents by an `Arc`. This `Arc` actually has
some interesting structure. Each of the edges in the diagram below
represents something that is "kept alive" by holding a ref count (in
some way, usually via an `Arc`):

      F' ---+  [ deque ] --+
            |              |
            v              v
      +---> /---------------------\
      |     | registry:           | ------> [rayon registry]
      |     | contents: --------\ |
      |     | | scope         | | ------> [spawning scope]
      |     | | unpark          | | --+
      |     | | this            | | --+ (self references)
      |     | | ...             | |   |
      |     | \-----------------/ |   |
      |     \---------------------/   |
      +-------------------------------+

Let's walk through them:

- The incoming edge from `F'` represents the edge from the future that was returned
  to the caller of `spawn_future`. This ensures that the future arc will
  not be freed so long as the caller is still interesting in looking at
  its result.
- The incoming edge from `[ deque ]` represents the fact that when the
  future is enqueued into a thread-local deque (which it only
  sometimes is), that deque holds a ref. This is done by transmuting
  the `Arc` into a `*const Job` object (and hence the `*const`
  logically holds the ref that was owned by the `Arc`).  When the job
  is executed, it is transmuted back and the resulting `Arc` is
  eventually dropped, releasing the ref.
- The `registry` field holds onto an `Arc<Registry>` and hence keeps
  some central registry alive. This doesn't really do much but prevent
  the `Registry` from being dropped. In particular, this doesn't
  prevent the threads in a registry from terminating while the future
  is unscheduled etc (though other fields in the future do).
- The `scope` field (of type `S`) is the "enclosing scope". This scope
  is an abstract value that implements the `FutureScope<'scope>` trait
  -- this means that it is responsible for ensuring that `'scope` does
  not end until one of the `FutureScope` methods are invoked (which
  occurs when the future has finished executing). For example, if the
  future is spawned inside a `scope()` call, then the `S` will be a
  wrapper (`ScopeFutureScope`) around a `*const Scope<'scope>`.  When
  the future is created one job is allocated for this future in the
  scope, and the scope counter is decremented once the future is
  marked as completing.
  - In general, the job of the `scope` field is to ensure that the
    future type (`F`) remains valid. After all, since `F: 'scope`, `F`
    is known to be valid until the lifetime `'scope` ends, and that
    lifetime cannot end until the `scope` methods are invoked, so we
    know that `F` must stay valid until one of those methods are
    invoked.
  - All of our data of type `F` is stored in the field `spawn` (not
    shown here). This field is always set to `None` before the scope
    counter is decremented. See the section on lifetime safety for more
    details.
- The `unpark` and `self` fields both store an `Arc` which is actually
  this same future. Thus the future has a ref count cycle (two of
  them...) and cannot be freed until this cycle is broken. Both of
  these fields are actually `Option<Arc<..>>` fields and will be set
  to `None` once the future is complete, breakin the cycle and
  allowing it to be freed when other references are dropped.

### The future state machine

Internally, futures go through various states, depicted here:

    PARKED <----+
    |           |
    v           |
    UNPARKED    |
    |           |
    v           |
    EXECUTING --+
    |   |   ^
    |   v   |
    |   EXECUTING_UNPARKED
    |
    v
    COMPLETE

When they are first created, futures begin as *PARKED*. A *PARKED*
future is one that is waiting for something to happen. It is not
scheduled in the deque of any thread. Even before we return from
`spawn_future()`, however, we will transition into *UNPARKED*. An
*UNPARKED* future is one that is waiting to be executed. It is
enqueued in the deque of some Rayon thread and hence will execute when
the thread gets around to it.

Once the future begins to execute (it itself is a Rayon job), it
transitions into the *EXECUTING* state. This means that it is busy
calling `F.poll()`, basically. While it calls `poll()`, it also sets
up its `contents.unpark` field as the current "unpark" instance. Hence
if `F` returns `NotReady`, it will clone this `unpark` field and hold
onto it to signal us the future is ready to execute again.

For now let's assume that `F` is complete and hence readys either
`Ok(Ready(_))` or `Err(_)`. In that case, the future can transition to
`COMPLETE`. At this point, many bits of state that are no longer
needed (e.g., the future itself, but also the `this` and `unpark`
fields) are set to `None` and dropped, and the result is stored in the
`result` field. (Moreover, we may have to signal other tasks, but that
is discussed in a future section.)

If `F` returns `Ok(Async::NotReady)`, then we would typically
transition to the `PARKED` state and await the call to
`unpark()`. When `unpark()` is called, it would move the future into
the `UNPARK` state and inject it into the registry.

However, due to the vagaries of thread-scheduling, it *can* happen
that `unpark()` is called before we exit the `EXECUTING` state. For
example, we might invoke `F.poll()`, which send the `Unpark` instance
to the I/O thread, which detects I/O, and invokes `unpark()`, all
before `F.poll()` has returned. In that case, the `unpark()` method
will transition the state (atomically, of course) to
`EXECUTING_UNPARKED`. In that case, instead of transitioning to
`PARKED` when `F.poll()` returns, the future will simply transition
right back to `EXECUTING` and try calling `poll()` again. This can
repeat a few times.

### Lifetime safety

Of course, Rayon's signature feature is that it allows you to use a
future `F` that includes references, so long as those references
outlive the lifetime of the scope `'scope`. So why is this safe?

The basic idea of why this is safe is as follows. The `ScopeFuture`
struct holds a ref on the scope itself (via the field `scope`).
Until this ref is decremented, the scope will not end (and hence
`'scope` is still active). This ref is only decremented while the
future transitions into the *COMPLETE* state -- so anytime before
then, we know we don't have to worry, the references are still valid.

As we transition into the *COMPLETE* state is where things get more
interesting. You'll notice that signaling the `self.scope` job as done
the *last* thing that happens during that transition. Importantly,
before that is done, we drop all access that we have to the type `F`:
that is, we store `None` into the fields that might reference values
of type `F`. This implies that we know that, whatever happens after we
transition into *COMPLETE*, we can't access any of the references
found in `F` anymore.

This is good, because there *are* still active refs to the
`ScopeFuture` after we enter the *COMPLETE* state. There are two
sources of these: unpark values and the future result.

**Unpark values.** We may have given away `Arc<Unpark>` values --
these are trait objects, but they are actually refs to our
`ScopeFuture`. Note that `Arc<Unpark>: 'static`, so these could be
floating about for any length of time (we had to transmute away the
lifetimes to give them out). This is ok because (a) the `Arc` keeps
the `ScopeFuture` alive and (b) the only thing you can do is to call
`unpark()`, which will promptly return since the state is *COMPLETE*
(and, anyhow, as we saw above, it doesn't have access to any
references anyhow).

**Future result.** The other, more interesting reference to the
`ScopeFuture` is the value that we gave back to the user when we
spawned the future in the first place. This value is more interesting
because it can be used to do non-trivial things, unlike the
`Arc<Unpark>`. If you look carefully at this handle, you will see that
its type has been designed to hide the type `F`. In fact, it only
reveals the types `T` and `E` which are the ok/err result types of the
future `F`.  This is intentonal: suppose that the type `F` includes
some references, but those references don't appear in the result. We
want the "result" future to be able to escape the scope, then, to any
place where the types `T` and `E` are still in scope. If we exposed
`F` here that would not be possible. (Hiding `F` also requires a
transmute to an object type, in this case an internal trait called
`ScopeFutureTrait`.) Note though that it is possible for `T` and `E`
to have references in them. They could even be references tied to the
scope.

So what can a user do with this result future? They have two
operations available: poll and cancel. Let's look at cancel first,
since it's simpler. If the state is *COMPLETE*, then `cancel()` is an
immediate no-op, so we know that it can't be used to access any
references that may be invalid. In any case, the only thing it does is
to set a field to true and invoke `unpark()`, and we already examined
the possible effects of `unpark()` in the previous section.q

So what about `poll()`? This is how the user gets the final result out
of the future. The important thing that it does is to access (and
effectively nullify) the field `result`, which stores the result of
the future and hence may have access to `T` and `E` values. These
values may contain references...so how we know that they are still in
scope?  The answer is that those types are exposed in the user's type
of the future, and hence the basic Rust type system should guarantee
that any references are still valid, or else the user shouldn't be
able to call `poll()`. (The same is true at the time of cancellation,
but that's not important, since `cancel()` doesn't do anything of
interest.)


