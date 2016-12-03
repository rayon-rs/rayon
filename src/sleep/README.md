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
compare. If there are sleepy workers, a `Release` store is needed.  If
there workers *asleep*, we must naturally acquire the lock and signal
the condition variable.

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










