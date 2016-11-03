use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;
use std::sync::TryLockError;
use std::thread;
use thread_pool::WorkerThread;
use util::PoisonPanic;

pub struct FlatCombiner<F, T>
    where T: Send,
          F: FnMut(T)
{
    senders: Vec<Sender<T>>,
    owner_data: Mutex<OwnerData<F>>,
}

struct Sender<T>
    where T: Send
{
    data: AtomicPtr<T>
}

struct OwnerData<F> {
    func: F,
}

#[must_use]
struct BecameOwner(bool);

impl BecameOwner {
    fn became_owner(&self) -> bool { self.0 }
}

impl<F, T> FlatCombiner<F, T>
    where T: Send,
          F: FnMut(T)
{
    /// Create a `FlatCombiner`.
    ///
    /// Unsafe because this must be called from inside a worker thread
    /// in the same registry as the one from which the later calls to
    /// `produce()` will occur.
    #[allow(dead_code)] // FIXME
    pub unsafe fn new(func: F) -> FlatCombiner<F, T> {
        let worker = WorkerThread::current();
        debug_assert!(!worker.is_null());
        let num_threads = (*worker).num_threads();

        FlatCombiner {
            senders: (0..num_threads).map(|_| Sender::new()).collect(),
            owner_data: Mutex::new(OwnerData { func: func }),
        }
    }

    /// Executes the closure on `data` and returns. This executes
    /// atomically with respect to all other calls to `produce()`.
    ///
    /// Unsafe because this must be called from inside a worker thread
    /// in the same registry as the one from which the
    /// `FlatCombiner::new()` method was invoked.
    pub unsafe fn produce(&self, data: T) {
        let worker = WorkerThread::current();
        debug_assert!(!worker.is_null());

        let data = Some(data); // meh

        let index = (*worker).index();
        let sender = &self.senders[index];

        // We have not yet enqueued any data, so there can't be any.
        debug_assert!(sender.is_empty());

        // Otherwise, enqueue the data.
        self.send(sender, data.unwrap());
    }

    unsafe fn send(&self,
                   sender: &Sender<T>,
                   mut data: T)
    {
        // At this point, the data is owner by the queue, so forget
        // it. The `data_guard` that is returned by the queue will
        // ensure we free the data if a panic should occur.
        let data_ptr: *mut T = &mut data;
        mem::forget(data);
        let data_guard = sender.put(data_ptr);

        loop {
            if self.try_to_become_owner().became_owner() {
                // If we became owner, then all data
                // would be processed, and so `sender.is_empty()`
                // must be true, and the `data_guard` is not needed.
                debug_assert!(sender.is_empty());
                mem::forget(data_guard);
                return;
            }

            if sender.is_empty() {
                // Our request was served, so we can just return, and
                // `data_guard` is not needed.
                mem::forget(data_guard);
                return;
            }

            // Otherwise, spin.
            thread::yield_now();
        }
    }

    unsafe fn try_to_become_owner(&self) -> BecameOwner {
        match self.owner_data.try_lock() {
            Ok(mut owner_data) => {
                self.be_owner(&mut owner_data);
                BecameOwner(true)
            }
            Err(TryLockError::WouldBlock) => {
                BecameOwner(false)
            }
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

    unsafe fn be_owner(&self, owner_data: &mut OwnerData<F>) {
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
        for _ in 0 .. 3 {
            for sender in &self.senders {
                if let Some(data) = sender.try_take() {
                    (owner_data.func)(data);
                }
            }
        }
    }
}

impl<T> Sender<T>
    where T: Send
{
    fn new() -> Self {
        Sender {
            data: AtomicPtr::new(ptr::null_mut())
        }
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
    unsafe fn put(&self, data_ptr: *mut T) -> SenderGuard<T> {
        debug_assert!(self.is_empty());
        self.data.store(data_ptr, Ordering::SeqCst);
        SenderGuard { sender: self }
    }

    /// Try to take an item from the sender queue; returns `None` if
    /// nothing is present. Normally, this should only be executed by
    /// the owner, but in the case of panic there is some special
    /// handling; see `SenderGuard` for details.
    ///
    /// Unsafe because it should only be executed by the owner (or a properly used `SenderGuard`).
    unsafe fn try_take(&self) -> Option<T> {
        let data_ptr = self.data.swap(ptr::null_mut(), Ordering::SeqCst);
        if data_ptr.is_null() {
            None
        } else {
            Some(ptr::read(data_ptr))
        }
    }

    /// Check if there is currently data in the queue. Safe.
    fn is_empty(&self) -> bool {
        self.data.load(Ordering::SeqCst).is_null()
    }
}

struct SenderGuard<'sender, T: 'sender>
    where T: Send
{
    sender: &'sender Sender<T>
}

impl<'sender, T> Drop for SenderGuard<'sender, T>
    where T: Send
{
    fn drop(&mut self) {
        unsafe {
            // Subtle safety point: normally, `try_take()` should only
            // be executed by the *owner* of the lock. But we are
            // guaranteed by the structure of the algorithm that if
            // this guard executes, one of two things have happened:
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
            self.sender.try_take(); // will drop if there was any data present
        }
    }
}

