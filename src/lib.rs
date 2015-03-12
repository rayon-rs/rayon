#![feature(unsafe_destructor)]

use std::thread;

mod test;

pub type TaskBody<'s> = &'s mut (FnMut() + Sync + Send);

pub struct Section<'s> {
    tasks: Vec<thread::JoinGuard<'s, ()>>,
}

pub fn execute<'s>(closures: &'s mut [TaskBody]) {
    let mut join = Section::new();
    for closure in closures.iter_mut() {
        join.fork(closure);
    }
    join.sync();
}

impl<'s> Section<'s> {
    pub fn new() -> Section<'s> {
        Section { tasks: Vec::new() }
    }

    pub fn fork(&mut self, body: &'s mut TaskBody) {
        // really don't want the `push` below to fail
        // after task has been spawned
        self.tasks.reserve(1);

        let future = thread::scoped(move || {
            (*body)()
        });

        // due to reserve above, should be infallible
        self.tasks.push(future);
    }

    pub fn sync(&mut self) {
        loop {
            match self.tasks.pop() {
                None => { break; }
                Some(joinguard) => {
                    // propoagate any failure
                    joinguard.join();
                }
            }
        }
    }
}

#[unsafe_destructor]
impl<'s> Drop for Section<'s> {
    fn drop(&mut self) {
        self.sync();
    }
}
