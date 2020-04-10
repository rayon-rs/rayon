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

An **active** thread is one that is running tasks and doing work.

An **idle** thread is one that is in a busy loop looking for work. It will be
actively trying to steal from other threads and searching the global injection
queue for jobs. While it does this, it will periodically call back into the
sleep module with information about its progress.

Towards the end of the idle period, idle threads also become **sleepy**. A
**sleepy** thread is an idle thread that is *about* to go to sleep after it does
one more search (or some other number, potentially). The role of going sleepy is
to prevent a possible race condition, wherein:

* some thread A attempts to steal work from thread B, but finds that B's queue
  is empty;
* thread B posts work, but finds that no workers are sleeping and hence there is
  no one to wake;
* thread A decides to go to sleep, and thus never sees the work that B posted.

This race condition can lead to suboptimal performance because we have no
workers available.

A **sleeping** thread is one that is blocked and which must be actively awoken.
Sleeping threads will always be blocked on the mutex/condition-variable assigned
to them in the `Sleep` struct.

## The counters

One of the key structs in the sleep module is `AtomicCounters`, found in
`counters.rs`. It packs four counters into one atomically managed value. These
counters fall into two categories:

* **Thread counters**, which track the number of threads in a particular state.
* **Event counters**, which track the number of events that occurred.

### Thread counters

There are two thread counters, one that tracks **idle** threads and one that
tracks **sleeping** threads. These are incremented and decremented as threads
enter and leave the idle/sleeping state in a fairly straightforward fashion. It
is important, however, that the thread counters are maintained atomically with
the event counters to prevent race conditions between new work being posted and
threads falling asleep. In other words, when new work is posted, it may need to
wake sleeping threads, and so we need a clear ordering whether the *work was
posted first* or the *thread fell asleep first*.

### Event counters

There are two event counters, the **jobs event counter** and the **sleepy
event counter**. Event counters, unlike thread counters, are never
decremented (modulo rollover, more on that later). They are simply advanced forward each time some event occurs.

The jobs event counter is, **conceptually**, incremented each time a new job is posted. The role of this is that, when a thread becomes sleepy, it can read the jobs event counter to see how many jobs were posted up till that point. Then, when it goes to sleep, it can atomically do two things:

* Verify that the event counter has not changed, and hence no new work was posted;
* Increment the sleepy thread counter, so that if any new work comes later, it
  will know that there are sleeping threads that may have to be awoken.

This simple picture, however, isn't quite what we do -- the problem is two-fold.
First, it would require that each time a new job is posted, we do a write
operation to a single global counter, which would be a performance bottleneck
(how much of one? I'm not sure, measurements would be good!).

Second, it would not account for rollover -- it is possible, after all, that
enough events have been posted that the counter **has** rolled all the way back
to its original value!

To address the first point, we keep a separate **sleepy event counter** that
(sort of) tracks how many times threads have become sleepy. Both event counters
begin at zero. Thus, after a new job is posted, we can check if they are still
equal to one another. If so, we can simply post our job, and we can now say that
the job was posted *before* any subsequent thread got sleepy -- therefore, if a
thread becomes sleepy after that, that thread will find the job when it does its
final search for work.

If however we see that the sleepy event counter (SEC) is **not** equal to the jobs event counter (JEC), that indicates that some thread has become sleepy since work was last posted. This means we have to do more work. What we do, specifically, is to set the JEC to be equal to the SEC, and we do this atomically.

Meanwhile, the sleepy thread, before it goes to sleep, will read the counters again and check that the JEC has not changed since it got sleepy. So, if new work *was* posted, then the JEC will have changed (to be equal to the SEC), and the sleepy thread will go back to the idle state and search again (and then possibly become sleepy again).

This is the high-level idea, anyway. Things get subtle when you consider that there are many threads, and also that we must account for rollover.

## Actual procedure

Let's document the precise procedure we follow, first. In each case, all the operations happen atomically, which is easy to implement through a compare and swap. 

* To announce a thread is idle:
  * IdleThreads += 1
* To announce a thread found work:
  * IdleThreads -= 1
  * If IdleThreads == SleepingThreads, return NeedToWake
* To announce a thread is sleepy:
  * If SleepyEventCounter == MAX, set JobsEventCounter = SleepyEventCounter = 0
  * Else, set SleepyEventCounter = SleepyEventCounter + 1
* To post work:
  * If JobsEventCounter != SleepyEventCounter:
    * JobsEventCounter = SleepyEventCounter
  * NeedToWake = decide(IdleThreads, SleepingThreads)
* To fall asleep:
  * If JobsEventCounter > SleepyEventCounter:
    * become idle
  * SleepingThreads += 1

### Accounting for rollover

Let's consider three threads, A, B, and C:

* A becomes sleepy, incrementing SleepyEventCounter to get SEC_A (which is MAX)
* In a loop:
  * B gets sleepy, reseting SleepyEventCounter to 0
  * B posts work,
