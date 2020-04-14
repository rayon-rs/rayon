# Introduction: the sleep module

The code in this module governs when worker threads should go to
sleep. The system used in this code was introduced in [Rayon RFC #5].
There is also a [video walkthrough] available. Both of those may be
valuable resources to understanding the code, though naturally they
will also grow stale over time. The comments in this file are
extracted from the RFC and meant to be kept up to date.

[Rayon RFC #5]: https://github.com/rayon-rs/rfcs/pull/5
[video walkthrough]: https://youtu.be/HvmQsE5M4cY

## The `Sleep` struct

The `Sleep` struct is embedded into each registry. It performs several functions:

* It tracks when workers are awake or asleep.
* It decides how long a worker should look for work before it goes to sleep,
  via a callback that is invoked periodically from the worker's search loop.
* It is notified when latches are set, jobs are published, or other
  events occur, and it will go and wake the appropriate threads if
  they are sleeping.

## Thread states

There are three main thread states:

* An **active** thread is one that is actively executing a job.
* An **idle** thread is one that is searching for work to do. It will be
  trying to steal work or pop work from the global injector queue.
* A **sleeping** thread is one that is blocked on a condition variable,
  waiting to be awoken.

We sometimes refer to the final two states collectively as **inactive**.
Threads begin as idle but transition to idle and finally sleeping when
they're unable to find work to do.

### Sleepy threads

There is one other special state worth mentioning. During the idle state,
threads can get **sleepy**. A sleepy thread is still idle, in that it is still
searching for work, but it is *about* to go to sleep after it does one more
search (or some other number, potentially). When a thread enters the sleepy
state, it signals (via the **jobs event counter**, described below) that it is
about to go to sleep. If new work is published, this will lead to the counter
being adjusted. When the thread actually goes to sleep, it will (hopefully, but
not guaranteed) see that the counter has changed and elect not to sleep, but
instead to search again. See the section on the **jobs event counter** for more
details.

## The counters

One of the key structs in the sleep module is `AtomicCounters`, found in
`counters.rs`. It packs three counters into one atomically managed value:

* Two **thread counters**, which track the number of threads in a particular state.
* The **jobs event counter**, which is used to signal when new work is available.
  It (sort of) tracks the number of jobs posted, but not quite, and it can rollover.

### Thread counters

There are two thread counters, one that tracks **inactive** threads and one that
tracks **sleeping** threads. From this, one can deduce the number of threads
that are idle by subtracting sleeping threads from inactive threads. We track
the counters in this way because it permits simpler atomic operations. One can
increment the number of sleeping threads (and thus decrease the number of idle
threads) simply by doing one atomic increment, for example. Similarly, one can
decrease the number of sleeping threads (and increase the number of idle
threads) through one atomic decrement.

These counters are adjusted as follows:

* When a thread enters the idle state: increment the inactive thread counter.
* When a thread enters the sleeping state: increment the sleeping thread counter.
* When a thread awakens a sleeping thread: decrement the sleeping thread counter.
  * Subtle point: the thread that *awakens* the sleeping thread decrements the
    counter, not the thread that is *sleeping*. This is because there is a delay
    between siganling a thread to wake and the thread actually waking:
    decrementing the counter when awakening the thread means that other threads
    that may be posting work will see the up-to-date value that much faster.
* When a thread finds work, exiting the idle state: decrement the inactive
  thread counter.

### Jobs event counter

The final counter is the **jobs event counter**. The role of this counter
is to help sleepy threads detect when new work is posted in a lightweight
fashion. It is important if the counter is even or odd:

* When the counter is **even**, it means that the last thing to happen was
  that new work was published.
* When the counter is **odd**, it means that the last thing to happen was
  that some thread got sleepy.

It works like so:

* When a thread becomes sleepy, it checks if the counter is even. If so,
  it remembers the value (`JEC_OLD`) and increments so that the counter
  is odd.
* When a thread publishes work, it checks if the counter is odd. If so,
  it increments the counter (potentially rolling it over).
* When a sleepy thread goes to sleep, it reads the counter again. If
  the counter value has changed from `JEC_OLD`, then it knows that
  work was published during its sleepy time, so it goes back to the idle
  state to search again.

Assuming no rollover, this protocol serves to prevent a race condition
like so:

* Thread A gets sleepy.
* Thread A searches for work, finds nothing, and decides to go to sleep.
* Thread B posts work, but sees no sleeping threads, and hence no one to wake up.
* Thread A goes to sleep, incrementing the sleeping thread counter.

The race condition would be prevented because Thread B would have incremented
the JEC, and hence Thread A would not actually go to sleep, but rather return to
search again.

However, because of rollover, the race condition cannot be completely thwarted.
It is possible, if exceedingly unlikely, that Thread A will get sleepy and read
a value of the JEC. And then, in between, there will be *just enough* activity
from other threads to roll the JEC back over to precisely that old value.

## Protocol to fall asleep

The full protocol for a thread to fall asleep is as follows:

* The thread "announces it is sleepy" by incrementing the JEC if it is even.
* The thread searches for work. If work is found, it becomes active.
* If no work is found, the thread atomically:
  * Checks the JEC to see that it hasn't changed. If it has, then the thread
    returns to *just before* the "sleepy state" to search again (i.e., it won't
    search for a full set of rounds, just a few more times).
  * Increments the number of sleeping threads by 1.
* The thread then does one final check for injected jobs (see below). If any
  are available, it returns to the 'pre-sleepy' state as if the JEC had changed.
* The thread waits to be signaled. Once signaled, it returns to the idle state.

### The jobs event counter and deadlock

As described in the section on the JEC, the main concern around going to sleep
is avoiding a race condition wherein:

* Thread A looks for work, finds none.
* Thread B posts work but sees no sleeping threads.
* Thread A goes to sleep.

The JEC protocol largely prevents this, but due to rollover, this prevention is
not complete. It is possible -- if unlikely -- that enough activity occurs for
Thread A to observe the same JEC value that it saw when getting sleepy. If the
new work being published came from *inside* the thread-pool, then this race
condition isn't too harmful. It means that we have fewer workers processing the
work then we should, but we won't deadlock. This seems like an acceptable risk
given that this is unlikely in practice.

However, if the work was posted as an *external* job, that is a problem. In that
case, it's possible that all of our workers could go to sleep, and the external
job would never get processed.

To prevent that, the sleeping protocol includes one final check for external
jobs. Note that we are guaranteed to see any such jobs, but the reasoning is
actually pretty subtle.

The key point is that, if the JEC == JEC_OLD, then there are three possibilities:

* No intervening operation occurred (i.e., no job was posted). In that case,
  going to sleep means that any subsequent job will see (and wake) a sleeping
  thread, so no deadlock.
* Rollover occurred and JEC is odd: In this case, the last thing to happen was
  that some other thread became sleepy. That thread will see any jobs that were
  posted.
* Rollover occurred and JEC is even: In this case, the last thing to happen was
  that a job was posted.

* Thread B will first post the external job into the queue.
* Thread B will then check for sleeping threads, which is a sequentially consistent
  read.
* Thread A will increment the number of sleeping threads, which is also SeqCst.
* Thread A then reads the external job.

XXX this is false -- there is no synchronizes-with relation between a seq-cst
read and the increment on thread A. We could add fences to achieve the desired
effect.