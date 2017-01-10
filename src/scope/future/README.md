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
      |     | | counter         | | ------> [rayon scope's counter]
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
- The `counter` field is a `*const CountLatch`, but the idea is that
  when the future is created one "count" on this `CountLatch` is
  allocated for the future. Once the future has finished executing
  completely (and hence its data is ready), this count will be
  released. Typically, this `counter` is pointing at the `CountLatch`
  associated with a call to `scope()`, and hence also the latch that
  keeps the scope from ending. This has an interesting relationship to
  the future `F`, as the type `F` is only valid during the call to
  `scope()` (i.e., it may have references into the stack which become
  invalidated once `scope()` returns).
  - All of our data of type `F` is stored in the field `spawn` (not
    shown here). This field is always set to `None` before the counter
    is decremented. See the section on lifetime safety for more
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
