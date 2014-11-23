#![feature(unsafe_destructor)]

use std::any::Any;
use std::kinds::marker;
use std::mem::transmute;
use std::raw;
use std::sync::Future;
use std::task;

mod test;

pub type TaskBody<'s> = ||:Sync+'s;

pub struct Section<'s> {
    marker: marker::ContravariantLifetime<'s>,
    tasks: Vec<Future<Result<(), Box<Any+Send>>>>
}

pub fn execute<'s>(closures: &'s mut [TaskBody<'s>]) {
    let mut join = Section::new();
    for closure in closures.iter_mut() {
        join.fork(closure);
    }
    join.sync();
}

impl<'s> Section<'s> {
    pub fn new() -> Section<'s> {
        Section { marker: marker::ContravariantLifetime,
                  tasks: Vec::new() }
    }

    pub fn fork(&mut self, body: &'s mut TaskBody<'s>) {
        unsafe {
            let body: *mut raw::Closure = transmute(body);

            // really don't want the `push` below to fail
            // after task has been spawned
            self.tasks.reserve(1);

            let future = task::try_future(proc() {
                let body: &mut TaskBody = transmute(body);
                (*body)()
            });

            // due to reserve above, should be infallible
            self.tasks.push(future);
        }
    }

    pub fn sync(&mut self) {
        loop {
            match self.tasks.pop() {
                None => { break; }
                Some(task) => {
                    // propoagate any failure
                    match task.unwrap() {
                        Ok(()) => { }
                        Err(_) => { panic!() }
                    }
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

