# Introduction: the sleep module

The code in this module governs when worker threads should go to
sleep. This is a tricky topic -- the work-stealing algorithm relies on
having active worker threads running around stealing from one
another. But, if there isn't a lot of work, this can be a bit of a
drag, because it requires high CPU usage.

The code in this module takes a fairly simple approach to the
problem. It allows worker threads to fall asleep if they have failed
to steal work after various thresholds; however, whenever new work
appears, they will wake up briefly and try to steal again. There are some
shortcomings in this current approach:

- it can (to some extent) scale *down* the amount of threads, but they
  can never be scaled *up*. The latter might be useful in the case of
  user tasks that must (perhaps very occasionally and unpredictably)
  block for long periods of time.
  - however, the preferred approach to this is for users to adopt futures
    instead (and indeed this sleeping work is intended to enable future
    integration).
- we have no way to wake up threads in a fine-grained or targeted
  manner. The current system wakes up *all* sleeping threads whenever
  *any* of them might be interested in an event. This means that while
  we can scale CPU usage down, we do is in a fairly "bursty" manner,
  where everyone comes online, then some of them go back offline.

# The interface for workers

Workers interact with the sleep module by invoking three methods:

- `work_found()`: signals that the worker found some work and is about
  to execute it.
- `no_work_found()`: signals that the worker searched all available sources
  for work and found none.
  - It is important for the coherence of the algorithm that if work
    was available **before the search started**, it would have been
    found. If work was made available during the search, then it's ok that
    it might have been overlooked.
- `tickle()`: indicates that new work is available (e.g., a job has
  been pushed to the local deque) or that some other blocking
  condition has been resolved (e.g., a latch has been set). Wakes up any
  sleeping workers.

When in a loop searching for work, Workers also have to maintain an
integer `yields` that they provide to the `sleep` module (which will
return a new value for the next time). Thus the basic worker "find
work" loop looks like this (this is `wait_until()`, basically):

```rust
let mut yields = 0;
while /* not done */ {
    if let Some(job) = search_for_work() {
        yields = work_found(self.index, yields);
    } else {
        yields = no_work_found(self.index, yields);
    }
}
```

# Getting sleepy and falling asleep

The basic idea here is that every worker goes through three states:

- **Awake:** actively hunting for tasks.
- **Sleepy:** still actively hunting for tasks, but we have signaled that
  we might go to sleep soon if we don't find any.
- **Asleep:** actually asleep (blocked on a condition variable).

At any given time, only **one** worker can be in the sleepy
state. This allows us to coordinate the entire sleep protocol using a
single `AtomicUsize` and without the need of epoch counters or other
things that might rollover and so forth.

Whenever a worker invokes `work_found()`, it transitions back to the
**awake** state. In other words, if it was sleepy, it stops being
sleepy. (`work_found()` cannot be invoked when the worker is asleep,
since then it is not doing anything.)

On the other hand, whenever a worker invokes `no_work_found()`, it
*may* transition to a more sleepy state. To track this, we use the
counter `yields` that is maintained by the worker's steal loop. This
counter starts at 0. Whenever work is found, the counter is returned
to 0. But each time that **no** work is found, the counter is
incremented. Eventually it will reach a threshold
`ROUNDS_UNTIL_SLEEPY`.  At this point, the worker will try to become
the sleepy one. It does this by executing a CAS into the global
registry state (details on this below). If that attempt is successful,
then the counter is incremented again, so that it is equal to
`ROUNDS_UNTIL_SLEEPY + 1`. Otherwise, the counter stays the same (and
hence we will keep trying to become sleepy until either work is found
or we are successful).

Becoming sleepy does not put us to sleep immediately. Instead, we keep
iterating and looking for work for some further number of rounds.  If
during this search we **do** find work, then we will return the
counter to 0 and also reset the global state to indicate we are no
longer sleepy.

But if again no work is found, `yields` will eventually reach the
value `ROUNDS_UNTIL_ASLEEP`. At that point, we will try to transition
from **sleepy** to **asleep**. This is done by the helper fn
`sleep()`, which executes another CAS on the global state that removes
our worker as the sleepy worker and instead sets a flag to indicate
that there are sleeping workers present (the flag may already have
been set, that's ok). Assuming that CAS succeeds, we will block on a
condition variable.

# Tickling workers

Of course, while all the stuff in the previous section is happening,
other workers are (hopefully) producing new work. There are three kinds of
events that can allow a blocked worker to make progress:

1. A new task is pushed onto a worker's deque. This task could be stolen.
2. A new task is injected into the thread-pool from the outside. This
   task could be uninjected and executed.
3. A latch is set. One of the sleeping workers might have been waiting for
   that before it could go on.

Whenever one of these things happens, the worker (or thread, more generally)
responsible must invoke `tickle()`. Tickle will basically wake up **all**
the workers:

- If any worker was the sleepy one, then the global state is changed
  so that there is no sleepy worker. The sleepy one will notice this
  when it next invokes `no_work_found()` and return to the *awake* state
  (with a yield counter of zero).
- If any workers were actually **asleep**, then we invoke
  `notify_all()` on the condition variable, which will cause them to
  awaken and start over from the awake state (with a yield counter of
  zero).

Because `tickle()` is invoked very frequently -- and hopefully most of
the time it is not needed, because the workers are already actively
stealing -- it is important that it be very cheap. The current design
requires, in the case where nobody is even sleepy, just a load and a
compare. If there are sleepy workers, a swap is needed.  If there
workers *asleep*, we must naturally acquire the lock and signal the
condition variable.

# The global state

We manage all of the above state transitions using a small bit of global
state (well, global to the registry). This is stored in the `Sleep` struct.
The primary thing is a single `AtomicUsize`. The value in this usize packs
in two pieces of information:

1. **Are any workers asleep?** This is just one bit (yes or no).
2. **Which worker is the sleepy worker, if any?** This is a worker id.

We use bit 0 to indicate whether any workers are asleep. So if `state
& 1` is zero, then no workers are sleeping. But if `state & 1` is 1,
then some workers are either sleeping or on their way to falling
asleep (i.e., they have acquired the lock).

The remaining bits are used to store if there is a sleepy worker. We
want `0` to indicate that there is no sleepy worker. If there a sleepy
worker with index `worker_index`, we would store `(worker_index + 1)
<< 1` . The `+1` is there because worker indices are 0-based, so this
ensures that the value is non-zero, and the shift skips over the
sleepy bit.

Some examples:

- `0`: everyone is awake, nobody is sleepy
- `1`: some workers are asleep, no sleepy worker
- `2`: no workers are asleep, but worker 0 is sleepy (`(0 + 1) << 1 == 2`).
- `3`: some workers are asleep, and worker 0 is sleepy.

# Correctness level 1: avoiding deadlocks etc

In general, we do not want to miss wakeups. Two bad things could happen:

- **Suboptimal performance**: If this is a wakeup about a new job being
  pushed into a local deque, it won't deadlock, but it will cause
  things to run slowly. The reason that it won't deadlock is that we
  know at least one thread is active (the one doing the pushing), and
  it will (sooner or later) try to pop this item from its own local
  deque.
- **Deadlocks:** If this is a wakeup about an injected job or a latch that got set, however,
  this can cause deadlocks. In the former case, if a job is injected but no thread ever
  wakes to process it, the injector will likely block forever. In the latter case,
  imagine this scenario:
  - thread A calls join, forking a task T1, then executing task T2
  - thread B steals T1, forks a task T3, and executes T4.
  - thread A completes task T2 and blocks on T1
  - thread A steals task T3 from thread B
  - thread B finishes T4 and goes to sleep, blocking on T3
  - thread A completes task T3 and makes a wakeup, but it gets lost
  At this point, thread B is still asleep and will never signal T2, so thread A will itself
  go to sleep. Bad.

It turns out that guaranteeing we don't miss a wakeup while retaining
good performance is fairly tricky. This is because of some details of
the C++11 memory model. But let's ignore those for now and generally
assume sequential consistency. In that case, our scheme should work
perfectly.

Even if you assume seqcst, though, ensuring that you don't miss
wakeups can be fairly tricky in the absence of a central queue. For
example, consider the simplest scheme: imagine we just had a boolean
flag indicating whether anyone was asleep. Then you could imagine that
when workers find no work, they flip this flag to true. When work is
published, if the flag is true, we issue a wakeup.

The problem here is that checking for new work is not an atomic
action. So it's possible that worker 1 could start looking for work
and (say) see that worker 0's queue is empty and then search workers
2..N.  While that searching is taking place, worker 0 publishes some
new work.  At the time when the new work is published, the "anyone
sleeping?" flag is still false, so nothing happens. Then worker 1, who
failed to find any work, goes to sleep --- completely missing the wakeup!

We use the "sleepy worker" idea to sidestep this problem. Under our
scheme, instead of going right to sleep at the end, worker 1 would
become sleepy.  Worker 1 would then do **at least** one additional
scan. During this scan, they should find the work published by worker
0, so they will stop being sleepy and go back to work (here of course
we are assuming that no one else has stolen the worker 0 work yet; if
someone else stole it, worker 1 may still go to sleep, but that's ok,
since there is no more work to be had).

Now you may be wondering -- how does being sleepy help? What if,
instead of publishing its job right before worker 1 became sleepy,
worker 0 wait until right before worker 1 was going to go to sleep? In
other words, the sequence was like this:

- worker 1 gets sleepy
- worker 1 starts its scan, scanning worker 0's deque
- worker 0 publishes its job, but nobody is sleeping yet, so no wakeups occur
- worker 1 finshes its scan, goes to sleep, missing the wakeup

The reason that this doesn't occur is because, when worker 0 publishes
its job, it will see that there is a sleepy worker. It will clear the
global state to 0.  Then, when worker 1 its scan, it will notice that
it is no longer sleepy, and hence it will not go to sleep. Instead it
will awaken and keep searching for work.

The sleepy worker phase thus also serves as a cheap way to signal that
work is around: instead of doing the whole dance of acquiring a lock
and issuing notifications, when we publish work we can just swap a
single atomic counter and let the sleepy worker notice that on their
own.

## Beyond seq-cst

Unfortunately, the C++11 memory model doesn't generally guarantee
seq-cst. And, somewhat annoyingly, it's not easy for the sleep module
**in isolation** to guarantee the properties the need. The key
challenge has to do with the *synchronized-with* relation. Typically,
we try to use acquire-release reasoning, and in that case the idea is
that **if** a load observes a store, it will also observe those writes
that preceded the store. But nothing says that the load **must**
observe the store -- at least not right away.

The place that this is most relevant is the load in the `tickle()`
routine. The routine begins by reading from the global state. If it
sees anything other than 0, it then does a swap and -- if necessary --
acquires a lock and does a notify. This load is a seq-cst load (as are
the other accesses in tickle). This ensures that it is sensible to
talk about a tickle happening *before* a worker gets sleepy and so
forth.

It turns out that to get things right, if we use the current tickle
routine, we have to use seq-cst operations **both in the sleep module
and when publishing work**. We'll walk through two scenarios to
show what I mean. 

### Scenario 1: get-sleepy-then-get-tickled

This scenario shows why the operations in sleep must be seq-cst. We
want to ensure that once a worker gets sleepy, any other worker that
does a tickle will observe that. In other words, we want to ensure
that the following scenario **cannot happen**:

1. worker 1 is blocked on latch L
2. worker 1 becomes sleepy
    - becoming sleepy involves a CAS on the global state to set it to 4 ("worker 1 is sleepy")
3. worker 0 sets latch L
4. worker 0 tickles **but does not see that worker 0 is sleepy**

Let's diagram this. The notation `read_xxx(A) = V` means that a read
of location `A` was executed with the result `V`. The `xxx` is the
ordering and the location `A` is either `L` (latch) or `S` (global
state). I will leave the ordering on the latch as `xxx` as it is not
relevant here. The numbers correspond to the steps above.

```
    worker 0                    worker 1
 |                           +- 2: cas_sc(S, 4)
s|  3: write_xxx(L)          +
b|  4: read_sc(S) = ??? <-sc-+
 v
```

Clearly, this cannot happen with sc orderings, because read 4 will
always return `4` here. However, if we tried to use acquire-release
orderings on the global state, then there would be **no guarantee**
that the tickle will observe that a sleepy worker occurred. We would
be guaranteed only that worker 0 would **eventually** observe that
worker 1 had become sleepy (and, at that time, that it would see other
writes). But it could take time -- and if we indeed miss that worker 1
is sleepy, it could lead to deadlock or loss of efficiency, as
explained earlier.

### Scenario 2: tickle-then-get-sleepy

<a name="tickle-then-get-sleepy"></a>

This scenario shows why latch operations must *also* be seq-cst (and,
more generally, any operations that publish work before a tickle). We
wish to ensure that this ordering of events **cannot occur**:

1. worker 1 is blocked on latch L
2. worker 1 reads latch L, sees false, starts searching for work
3. worker 0 sets latch L
4. worker 0 tickles
    - the tickle reads from the global state, sees 0
5. worker 1 finishes searching, becomes sleepy
    - becoming sleepy involves a CAS on the global state to set it to 4 ("worker 1 is sleepy")
6. worker 1 reads latch L **but does not see that worker 0 set it**
7. worker 1 may then proceed to become sleepy

In other words, we want to ensure that if worker 0 sets a latch and
does a tickle *before worker 1 gets sleepy*, then worker 1 will
observe that latch as set when it calls probe. We'll see that, with
the current scheme, this implies that the latch memory orderings must
be seq-cst as well.

Here is the diagram:

```
    worker 0                    worker 1
 |                              2: read_xxx(L) = false
s|  3: write_xxx(L, true)
b|  4: read_sc(S) = 0 -+
 |                     +-sc---> 5: cas_sc(S, 4)
 v                              6: read_xxx(L) = ???
```

The diagram shows that each thread's actions are related by
*sequenced-before* (sb). Moreover the read and write of `S` are
related by `sc` (the seq-cst ordering). However, and this is crucial,
this **does not** imply that oper 4 *synchronizes-with* oper 5. This
is because a read never synchronizes-with a store, only the
reverse. Hence, if the latch were using acq-rel orderings, it would be
legal for oper 6 to return false. But if the latch were to use an
**sc** ordering itself, then we know that oper 6 must return true,
since `3 -sc-> 4 -sc-> 5 -sc-> 6`.

**Note** that this means that, before we tickle, we must execute some
seq-cst stores to publish our work (and during the scan we must load
from those same locations) **if we wish to guarantee that the work we
published WILL be seen by the other threads** (as opposed to
*may*). This is true for setting a latch -- if a latch is set but
another thread misses it, then the system could deadlock. However, in
the case of pushing new work to a deque, we choose not to use a seqcst
ordering. This is for several reasons:

- If we miss a wakeup, the consequences are less dire: we simply run
  less efficiently (after all, the current thread will eventually
  complete its current task and pop the new task off the deque).
- It is inconvenient: The deque code is beyond our control (it lies in another package). However,
  we could create a dummy `AtomicBool` for each deque and do a seqcst write to it
  (with whatever value) after we push to the deque, and a seqcst load whenever
  we steal from the deque.
- The cost of using a dummy variable was found to be quite high for some benchmarks:
  - 8-10% overhead on nbody-parreduce
  - 15% overhead on increment-all
  - 40% overhead on join-recursively

### Alternative solutions

In both cases above, our problems arose because tickle is merely
performing a seq-cst read. If we instead had tickle perform a release
*swap*, that would be a write action of the global state. No matter
the ordering mode, all writes to the same memory location have a total
ordering, and hence we would not have to worry about others storing a
value that we fail to read (as in scenario 1). Similarly, as a release
write, a swap during tickle would synchronize-with a later cas and so
scenario 2 should be averted. So you might wonder why we don't do
that. The simple reason was that it didn't perform as well! In my
measurements, many benchmarks were unaffected by using a swap, but
some of them were hit hard:
  - 8-10% overhead on nbody-parreduce
  - 35% overhead on increment-all
  - 245% overhead on join-recursively
