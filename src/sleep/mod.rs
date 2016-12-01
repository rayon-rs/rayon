//! Code that decides when workers should go to sleep. See README.md
//! for an overview.

use log::Event::*;
use std::sync::atomic::AtomicUsize;
use std::sync::{Condvar, Mutex};
use std::thread;
use std::usize;
use std::sync::atomic::Ordering::*;

// SUBTLE CORRECTNESS POINTS
//
// - Can't afford a "lost" tickle, because if thread X gets sleepy
//   and then misses a tickle, that might be the tickle to indicate that
//   its latch is set.
// - Sleeping while a latch is held: bad

/// The "epoch" is used to handle thread activity. The idea is that we
/// want worker threads to start to spin down when there is nothing to
/// do, but to spin up quickly.
pub struct Sleep {
    state: AtomicUsize,
    data: Mutex<()>,
    tickle: Condvar,
}

const AWAKE: usize = 0;
const SLEEPING: usize = 1;

const ROUNDS_UNTIL_SLEEPY: usize = 32;
const ROUNDS_UNTIL_ASLEEP: usize = 64;

impl Sleep {
    pub fn new() -> Sleep {
        Sleep {
            state: AtomicUsize::new(AWAKE),
            data: Mutex::new(()),
            tickle: Condvar::new(),
        }
    }

    fn anyone_sleeping(&self, state: usize) -> bool {
        state & SLEEPING != 0
    }

    fn any_worker_is_sleepy(&self, state: usize) -> bool {
        (state >> 1) != 0
    }

    fn worker_is_sleepy(&self, state: usize, worker_index: usize) -> bool {
        (state >> 1) == (worker_index + 1)
    }

    fn with_sleepy_worker(&self, state: usize, worker_index: usize) -> usize {
        debug_assert!(state == AWAKE || state == SLEEPING);
        ((worker_index + 1) << 1) + state
    }

    #[inline]
    pub fn work_found(&self, worker_index: usize, yields: usize) -> usize {
        log!(FoundWork { worker: worker_index, yields: yields });
        if yields > ROUNDS_UNTIL_SLEEPY {
            // FIXME tickling here is a bit extreme; mostly we want to "release the lock"
            // from us being sleepy, we don't necessarily need to wake others
            // who are sleeping
            self.tickle(worker_index);
        }
        0
    }

    #[inline]
    pub fn no_work_found(&self, worker_index: usize, yields: usize) -> usize {
        log!(DidNotFindWork { worker: worker_index, yields: yields });
        if yields < ROUNDS_UNTIL_SLEEPY {
            thread::yield_now();
            yields + 1
        } else if yields == ROUNDS_UNTIL_SLEEPY {
            if self.get_sleepy(worker_index) {
                yields + 1
            } else {
                yields
            }
        } else if yields < ROUNDS_UNTIL_ASLEEP {
            thread::yield_now();
            if self.still_sleepy(worker_index) {
                yields + 1
            } else {
                log!(GotInterrupted { worker: worker_index });
                0
            }
        } else {
            debug_assert_eq!(yields, ROUNDS_UNTIL_ASLEEP);
            self.sleep(worker_index);
            0
        }
    }

    pub fn tickle(&self, worker_index: usize) {
        let old_state = self.state.load(Acquire);
        if old_state != AWAKE {
            self.tickle_cold(worker_index);
        }
    }

    #[cold]
    fn tickle_cold(&self, worker_index: usize) {
        let old_state = self.state.swap(AWAKE, SeqCst);
        log!(Tickle { worker: worker_index, old_state: old_state });
        if self.anyone_sleeping(old_state) {
            let _data = self.data.lock().unwrap();
            self.tickle.notify_all();
        }
    }

    fn get_sleepy(&self, worker_index: usize) -> bool {
        loop {
            let state = self.state.load(SeqCst);
            log!(GetSleepy { worker: worker_index, state: state });
            if self.any_worker_is_sleepy(state) {
                // somebody else is already sleepy, so we'll just wait our turn
                debug_assert!(!self.worker_is_sleepy(state, worker_index),
                              "worker {} called `is_sleepy()`, \
                               but they are already sleepy (state={})",
                              worker_index, state);
                return false;
            } else {
                // make ourselves the sleepy one
                let new_state = self.with_sleepy_worker(state, worker_index);
                if self.state.compare_exchange(state, new_state, SeqCst, SeqCst).is_ok() {
                    log!(GotSleepy { worker: worker_index, old_state: state, new_state: new_state });
                    return true;
                }
            }
        }
    }

    fn still_sleepy(&self, worker_index: usize) -> bool {
        let state = self.state.load(SeqCst);
        self.worker_is_sleepy(state, worker_index)
    }

    fn sleep(&self, worker_index: usize) {
        loop {
            let state = self.state.load(SeqCst);
            if self.worker_is_sleepy(state, worker_index) {
                // It is important that we hold the lock when we do
                // the CAS. Otherwise, if we were to CAS first, then
                // the following sequence of events could occur:
                //
                // - Thread A (us) sets state to SLEEPING.
                // - Thread B sets state to AWAKE.
                // - Thread C sets state to SLEEPY(C).
                // - Thread C sets state to SLEEPING.
                // - Thread A reawakens, acquires lock, and goes to sleep.
                //
                // Now we missed the wake-up from thread B! But since
                // we have the lock when we set the state to sleeping,
                // that cannot happen. Note that the swap `tickle()`
                // is not part of the lock, though, so let's play that
                // out:
                //
                // # Scenario 1
                //
                // - A loads state and see SLEEPY(A)
                // - B swaps to AWAKE.
                // - A locks, fails CAS
                //
                // # Scenario 2
                //
                // - A loads state and see SLEEPY(A)
                // - A locks, performs CAS
                // - B swaps to AWAKE.
                // - A waits (releasing lock)
                // - B locks, notifies
                //
                // In general, acquiring the lock inside the loop
                // seems like it could lead to bad performance, but
                // actually it should be ok. This is because the only
                // reason for the `compare_exchange` to fail is if an
                // awaken comes, in which case the next cycle around
                // the loop will just return.
                let data = self.data.lock().unwrap();
                if self.state.compare_exchange(state, SLEEPING, SeqCst, SeqCst).is_ok() {
                    // Don't do this in a loop. If we do it in a loop, we need
                    // some way to distinguish the ABA scenario where the pool
                    // was awoken but before we could process it somebody went
                    // to sleep. Note that if we get a false wakeup it's not a
                    // problem for us, we'll just loop around and maybe get
                    // sleepy again.
                    log!(FellAsleep { worker: worker_index });
                    let _ = self.tickle.wait(data).unwrap();
                    log!(GotAwoken { worker: worker_index });
                    return;
                }
            } else {
                log!(GotInterrupted { worker: worker_index });
                return;
            }
        }
    }
}
