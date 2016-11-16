use caslist::CasList;
use registry::{RegistryId, WorkerThread};
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;
use std::sync::TryLockError;
use std::thread;
use util::PoisonPanic;

/**
`Atomic` executes a particular closure atomically, similar to holding
a [`Mutex`][mutex]. It is implemented using the [flat combining]
technique, which means that it can be much more efficient than a plain
mutex (but still less efficient than no synchronization at all).

# The `atomically!` macro

The easiest way to use `Atomic` is with the `atomically!` macro, so
let's start with some examples of that. (Note that to use Rayon macros
at all, you must annotate your `extern crate rayon` declaration with
`#[macro_use]`, as shown in the snippet below.)

```
// Declare `rayon` dependency in `lib.rs` or `main.rs` like so:
#[macro_use]
extern crate rayon;

use rayon::Atomic;
use rayon::prelude::*;
use std::collections::HashMap;

# fn main() {
// Store items into a hash map. Note that if
// there are duplicates, it is not defined which value will win,
// because `for_each` just executes in an arbitrary order.
let mut map: HashMap<i32, u32> = HashMap::new();
(0..1024_i32)
  .into_par_iter()
  .flat_map(|i| (0..1024_u32).into_par_iter().map(move |j| (i, j)))
  .for_each(atomically!(|(k, v)| {
    map.insert(k, v);
  }));
# }
```

# Creating and using `Atomic` instances manually

If the `atomically!` macro doesn't fit your needs, you can create an
`Atomic` instance yourself quite easily. The closure defines the
operation that will execute atomically; it should take one input
argument. For example, to create a closure that will insert each item
into a `HashMap`, you might do the following:

```
use rayon::Atomic;
use std::collections::HashMap;
let mut hashmap: HashMap<i32, u32> = HashMap::new();
let atomic_insert = Atomic::new(|(k, v)| hashmap.insert(k, v));
```

you can then invoke the closure by calling `invoke()`. The return
value of `invoke()` will be the return value of the closure:

```
# use rayon::Atomic;
# use std::collections::HashMap;
# let mut hashmap: HashMap<i32, u32> = HashMap::new();
# let atomic_insert = Atomic::new(|(k, v)| hashmap.insert(k, v));
let old_value = atomic_insert.invoke((22, 44));
assert!(old_value == None); // `insert()` should have returned `None`
let old_value = atomic_insert.invoke((22, 66));
assert!(old_value == Some(44)); // `insert()` would have returned `Some(44)`
```

[mutex]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
[flat combining]: https://www.cs.bgu.ac.il/~hendlerd/papers/flat-combining.pdf
*/
pub struct Atomic<F, T, R>
    where T: Send,
          R: Send,
          F: FnMut(T) -> R,
{
    registries: CasList<PerRegistry<T, R>>,
    owner: Mutex<Owner<F>>,
}

struct PerRegistry<T, R>
    where T: Send,
          R: Send,
{
    id: RegistryId,
    senders: Vec<Sender<T, R>>,
}

struct Sender<T, R>
    where T: Send,
          R: Send,
{
    // Three possible states, represented as follows:
    //
    // EMPTY: both `input` and `output` are null.
    //
    // - Transitions to SENT by writing (in this order!):
    //   - address of a mutable `Option<R>` slot into `output`
    //     - should be initialized with `None` to start
    //   - address of data into `input`
    //     - the owner now logically has ownership of this data
    //
    // SENT: both `input` and `output` are non-null.
    //
    // - Transitions to PROCESSING by writing null into `input`
    //
    // PROCESSING: `input` is null, `output` is not.
    //
    // - Transitions to SENT by writing null into `output`

    input: AtomicPtr<T>,
    output: AtomicPtr<Option<R>>, // where to write the result
}

struct Owner<F> {
    func: F,
}

#[must_use]
struct BecameOwner(bool);

impl BecameOwner {
   fn became_owner(&self) -> bool {
        self.0
    }
}

impl<F, T, R> Atomic<F, T, R>
    where T: Send,
          R: Send,
          F: FnMut(T) -> R
{
    /// Create a `Atomic`.
    #[allow(dead_code)] // FIXME
    pub fn new(func: F) -> Atomic<F, T, R> {
        Atomic {
            registries: CasList::new(),
            owner: Mutex::new(Owner { func: func }),
        }
    }

    /// Executes the closure on `data` and returns. This executes
    /// atomically with respect to all other calls to `invoke()`.
    ///
    /// Unsafe because this must be called from inside a worker thread
    /// in the same registry as the one from which the
    /// `Atomic::new()` method was invoked.
    pub fn invoke(&self, data: T) -> R {
        unsafe {
            let worker = WorkerThread::current();
            if !worker.is_null() {
                self.invoke_from_inside_worker(&*worker, data)
            } else {
                self.invoke_from_outside_worker(data)
            }
        }
    }

    /// Normal behavior: when in
    /// Unsafe contract: `worker` must be the current worker thread.
    unsafe fn invoke_from_inside_worker(&self, worker: &WorkerThread, data: T) -> R {
        let registry_id = worker.registry().id();
        if let Some(per_registry) = self.registries.iter().find(|r| r.id == registry_id) {
            self.invoke_from_recorded_registry(per_registry, worker, data)
        } else {
            self.invoke_from_unrecorded_registry(worker, data)
        }
    }

    /// We are in a worker thread that has been recorded with this
    /// `Atomic`. This is the happy path.
    ///
    /// Unsafe contract: `worker` must be the current worker thread;
    /// `per_registry` must be the corresponding registry.
    unsafe fn invoke_from_recorded_registry(&self,
                                            per_registry: &PerRegistry<T, R>,
                                            worker: &WorkerThread,
                                            mut data: T)
                                            -> R {
        debug_assert!(per_registry.id == worker.registry().id());

        let index = (*worker).index();
        let sender = &per_registry.senders[index];

        // We have not yet enqueued any data, so there can't be any.
        debug_assert!(sender.is_empty());

        let mut result_slot = None;

        // Transfer owneship of the data to the queue and forget about
        // it. The `data_guard` that is returned by the queue will
        // ensure we free the data if a panic should occur.
        let data_ptr: *mut T = &mut data;
        let result_ptr: *mut Option<R> = &mut result_slot;
        mem::forget(data);
        let data_guard = sender.put(data_ptr, result_ptr);

        loop {
            if self.try_to_become_owner().became_owner() {
                // If we became owner, then we should have processed
                // our own request (along with maybe some others).
                break;
            }

            if sender.is_empty() {
                // Our request was served, so we can just return, and
                // `data_guard` is not needed.
                break;
            }

            // Otherwise, spin.
            thread::yield_now();
        }

        // Either our request was served or we became the owner.
        // Either way, we do not want to free the data now.
        debug_assert!(sender.is_empty());
        mem::forget(data_guard);

        // Since we transitioned from SENT to PROCESSING and now back
        // to EMPTY, `result_slot` should have been written.
        result_slot.unwrap()
    }

    /// If this registry has never been recorded, then we should
    /// acquire the lock and add it. Note that we are contending with
    /// other workers in the same registry here.
    ///
    /// Unsafe contract: `worker` must be the current thread.
    #[cold]
    unsafe fn invoke_from_unrecorded_registry(&self, worker: &WorkerThread, data: T) -> R {
        let mut owner = self.owner.lock().unwrap();

        // add our registry to the list, if nobody else had done it yet
        let registry = worker.registry();
        if self.registries.iter().find(|r| r.id == registry.id()).is_none() {
            let senders =
                (0..registry.num_threads())
                .map(|_| Sender::new())
                .collect();
            self.registries.prepend(PerRegistry {
                id: registry.id(),
                senders: senders
            });
        }

        // process our item
        self.be_owner_with_data(&mut owner, data)
    }

    /// Fallback behavior: when invoked outside of a worker thread,
    /// just act like a regular mutex.
    fn invoke_from_outside_worker(&self, data: T) -> R {
        let mut owner = self.owner.lock().unwrap();

        // NB: It is debatable here if we should call `be_owner` or
        // just process the data and don't try to help others out.
        // Either way, the others will (presumably) make progress,
        // since one of them will become an owner. The reason not to
        // help others out is that, if we are not on a worker thread,
        // then likely there isn't enough work so there are no others
        // to even help! OTOH, in that case the list of registries is
        // empty, so calling `be_owner` is cheap, just a few loads.
        // And if there *are* others, then we really ought to play
        // nice.
        self.be_owner_with_data(&mut owner, data)
    }

    fn try_to_become_owner(&self) -> BecameOwner {
        match self.owner.try_lock() {
            Ok(mut owner) => {
                self.be_owner(&mut owner);
                BecameOwner(true)
            }
            Err(TryLockError::WouldBlock) => BecameOwner(false),
            Err(TryLockError::Poisoned(_)) => {
                // If some other worker thread panicked, then we just
                // want to propagate that panic. Ideally, we wouldn't
                // panic with a silly string here but instead some
                // kind of dummy value that the panic propagation code
                // could ignore.
                panic!(PoisonPanic);
            }
        }
    }

    fn be_owner_with_data(&self, owner: &mut Owner<F>, data: T) -> R {
        self.be_owner(owner);
        (owner.func)(data)
    }

    fn be_owner(&self, owner: &mut Owner<F>) {
        // Make 3 attempts to clear the queue before we pass the
        // baton. Why 3? No particular reason, just what I saw
        // elsewhere.
        //
        // Another possibility might be to wait until there are no
        // more requests pending. The problem I predict with that is
        // that it is rather unfair: the owner thread could be stuck
        // serving everybody else's requests and never producing any
        // of its own. It seems like it would interfere with our
        // load-balancing heuristics quite a bit.
        unsafe { // we assert that we are owner in `try_take_as_owner` and `release_as_owner`
            for _ in 0..3 {
                for sender in self.registries.iter().flat_map(|r| &r.senders) {
                    if let Some(input_data) = sender.try_take_as_owner() {
                        let output_data = (owner.func)(input_data);
                        sender.release_as_owner(output_data);
                    }
                }
            }
        }
    }
}

impl<T, R> Sender<T, R>
    where T: Send,
          R: Send,
{
    fn new() -> Self {
        Sender { input: AtomicPtr::new(ptr::null_mut()),
                 output: AtomicPtr::new(ptr::null_mut()) }
    }

    /// Enqueue an item into the sender queue. Ownership of the item
    /// conceptually passes into the queue; a guard is returned that,
    /// when dropped, will free the data if it has not yet been
    /// dequeued. But this guard must be used somewhat carefully,
    /// since the owner does not expect things from the queue to be
    /// removed willy nilly (this lets us use weaker ordering
    /// constraints than `SeqCst`). See the `drop()` method of
    /// `SenderGuard` for details.
    ///
    /// Unsafe because it should only ever be executed by the owner
    /// thread of this sender, and because of the memory ownership
    /// rules around `data_ptr`.
    unsafe fn put(&self, input_ptr: *mut T, output_ptr: *mut Option<R>) -> SenderGuard<T, R> {
        debug_assert!(self.is_empty());

        // These two stores "publish" the data to be processed as well
        // as the location to write the data to. Therefore, we use
        // "release" ordering so that the one who holds the lock will
        // be able to read data from `*input_ptr`.
        self.output.store(output_ptr, Ordering::Relaxed); // [A] must happen-before B
        self.input.store(input_ptr, Ordering::Release); // [B] synchronizes-with C
        SenderGuard { sender: self }
    }

    /// Try to take an item from the sender queue; returns `None` if
    /// nothing is present. This should only be executed by the owner.
    ///
    /// If something is returned, then the state moves from SENT to PROCESSING.
    ///
    /// Unsafe because it should only be executed by the owner.
    unsafe fn try_take_as_owner(&self) -> Option<T> {
        let input_ptr = self.input.swap(ptr::null_mut(), Ordering::Acquire);
        if input_ptr.is_null() {
            None
        } else {
            // At this state, the sender has stored some data and is
            // waiting for owner to clear it. As owner, we store a
            // sentinel in (to indicate we have taken ownership) and
            // then we read from the data. When we are fully done
            // processing it, we will store back a NULL to indicate
            // that sender can keep going (for now, they have to stick
            // around so that ptr to data remains valid).
            debug_assert!(!self.output.load(Ordering::Relaxed).is_null());
            Some(ptr::read(input_ptr))
        }
    }

    /// Owner calls this to indicate it has completed processing the given item.
    unsafe fn release_as_owner(&self, output_data: R) {
        debug_assert!(self.input.load(Ordering::Relaxed).is_null());
        let output = self.output.load(Ordering::Relaxed);
        debug_assert!(!output.is_null());
        *output = Some(output_data);
        self.output.store(ptr::null_mut(), Ordering::Release);
    }

    /// Try to take an item from the sender queue; returns `None` if
    /// nothing is present. Normally, this should only be executed by
    /// the owner, but in the case of panic there is some special
    /// handling; see `SenderGuard` for details.
    ///
    /// Unsafe because it should only be executed by the guard.
    ///
    /// # Memory ordering
    ///
    /// Uses relaxed reads because it is meant to be invoked on the
    /// same thread as the one which called `put`.
    unsafe fn try_take_by_guard(&self) -> Option<T> {
        let input_ptr = self.input.swap(ptr::null_mut(), Ordering::Relaxed);
        if input_ptr.is_null() {
            None
        } else {
            // since we're unwinding, nobody will be reading from the
            // output anyway, but we might as well leave the sender in
            // a valid state
            self.output.store(ptr::null_mut(), Ordering::Relaxed);
            Some(ptr::read(input_ptr))
        }
    }

    /// Check if there is currently data in the queue.
    ///
    /// **Memory ordering**
    ///
    /// This uses an `Acquire` on the output read because, if we have
    /// called `put`, then observing `is_empty()` being true indicates
    /// we can read from the output pointer.
    ///
    /// Similarly, we use `Relaxed` on the input because having
    /// observed `null` does not give us access to any data.
    fn is_empty(&self) -> bool {
        self.input.load(Ordering::Relaxed).is_null() && self.output.load(Ordering::Acquire).is_null()
    }
}

struct SenderGuard<'sender, T: 'sender, R: 'sender>
    where T: Send,
          R: Send,
{
    sender: &'sender Sender<T, R>,
}

impl<'sender, T, R> Drop for SenderGuard<'sender, T, R>
    where T: Send,
          R: Send,
{
    fn drop(&mut self) {
        unsafe {
            // Subtle safety point: We are guaranteed by the structure
            // of the algorithm that if this guard executes, one of
            // two things have happened:
            //
            // 1. The owner has panicked, and hence there will never
            //    be another owner, as the mutex is now poisoned. In this event,
            //    each worker thread is sort of "owner" of its own sender, so it
            //    is fine for us to touch it (since we know that it is the sender
            //    belonging to the current thread).
            // 2. We became owner, and hence we are guaranteed that the data is
            //    empty. Actually, if you read the code as it is written, this can't
            //    happen, because we always `mem::forget` the guard. But even if we didn't,
            //    it'd be fine, since we (the current thread) are the only ones that
            //    would ever enqueue into `self.sender`.
            self.sender.try_take_by_guard(); // will drop if there was any data present
        }
    }
}

/// A convenient wrapper for the [`Atomic`][atomic] struct. See
/// [`Atomic`][atomic] for documentation.
///
/// [atomic]: struct.Atomic.html
#[macro_export]
macro_rules! atomically {
    ($closure:expr) => {
        {
            let atomic = $crate::Atomic::new($closure);
            move |data| atomic.invoke(data)
        }
    }
}

#[cfg(test)]
mod test;

