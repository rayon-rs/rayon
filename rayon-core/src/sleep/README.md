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

The final counter is the **jobs event counter**. The role of this counter is to
help sleepy threads detect when new work is posted in a lightweight fashion. The
counter is incremented every time a new job is posted -- naturally, it can  also
rollover if there have been enough jobs posted.

The counter is used as follows:

* When a thread gets **sleepy**, it reads the current value of the counter.
* Later, before it goes to sleep, it checks if the counter has changed.
  If it has, that indicates that work was posted but we somehow missed it
  while searching. We'll go back and search again.

Assuming no rollover, this protocol serves to prevent a race condition
like so:

* Thread A gets sleepy.
* Thread A searches for work, finds nothing, and decides to go to sleep.
* Thread B posts work, but sees no sleeping threads, and hence no one to wake up.
* Thread A goes to sleep, incrementing the sleeping thread counter.

However, because of rollover, the race condition cannot be completely thwarted.
It is possible, if exceedingly unlikely, that Thread A will get sleepy and read
a value of the JEC. And then, in between, there will be *just enough* activity
from other threads to roll the JEC back over to precisely that old value. We
have an extra check in the protocol to prevent deadlock in that (rather
unlikely) case.

## Protocol for a worker thread to fall asleep

The full protocol for a thread to fall asleep is as follows:

* After completing all its jobs, the worker goes idle and begins to
  search for work. As it searches, it counts "rounds". In each round,
  it searches all other work threads' queues, plus the 'injector queue' for
  work injected from the outside. If work is found in this search, the thread
  becomes active again and hence restarts this protocol from the top.
* After a certain number of rounds, the thread "gets sleepy" and reads the JEC.
  It does one more search for work.
* If no work is found, the thread atomically:
  * Checks the JEC to see that it hasn't changed.
    * If it has, then the thread returns to *just before* the "sleepy state" to
      search again (i.e., it won't search for a full set of rounds, just a few
      more times).
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
job would never get processed. To prevent that, the sleeping protocol includes
one final check to see if the injector queue is empty before fully falling
asleep. Note that this final check occurs **after** the number of sleeping
threads has been incremented. We are not concerned therefore with races against
injections that occur after that increment, only before.

What follows is a "proof sketch" that the protocol is deadlock free. We model
two relevant bits of memory, the job injector queue J and the atomic counters C.

Consider the actions of the injecting thread:

* PushJob: Job is injected, which can be modeled as an atomic write to J with release semantics.
* IncJec: The JEC is incremented, which can be modeled as an atomic exchange to C with acquire-release semantics.

Meanwhile, the sleepy thread does the following:

* IncSleepers: The number of sleeping threads is incremented, which is atomic exchange to C with acquire-release semantics.
* ReadJob: We look to see if the queue is empty, which is a read of J with acquire semantics.

Both IncJec and IncSleepers modify the same memory location, and hence they must be fully ordered.

* If IncSleepers came first, there is no problem, because the injecting thread
  knows that everyone is asleep and it will wake up a thread.
* If IncJec came first, then it "synchronizes with" IncSleepers.
  * Therefore, PushJob "happens before" ReadJob, and so the write will be visible during
    this final check, and the thread will not go to sleep.
