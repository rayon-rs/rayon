use Configuration;
use {scope, Scope};
use ThreadPool;
use rand::{Rng, SeedableRng, XorShiftRng};
use std::cmp;
use std::iter::once;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use unwind;

#[test]
fn scope_empty() {
    scope(|_| {
    });
}

#[test]
fn scope_result() {
    let x = scope(|_| 22);
    assert_eq!(x, 22);
}

#[test]
fn scope_two() {
    let counter = &AtomicUsize::new(0);
    scope(|s| {
        s.spawn(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        s.spawn(move |_| {
            counter.fetch_add(10, Ordering::SeqCst);
        });
    });

    let v = counter.load(Ordering::SeqCst);
    assert_eq!(v, 11);
}

#[test]
fn scope_divide_and_conquer() {
    let counter_p = &AtomicUsize::new(0);
    scope(|s| s.spawn(move |s| divide_and_conquer(s, counter_p, 1024)));

    let counter_s = &AtomicUsize::new(0);
    divide_and_conquer_seq(&counter_s, 1024);

    let p = counter_p.load(Ordering::SeqCst);
    let s = counter_s.load(Ordering::SeqCst);
    assert_eq!(p, s);
}

fn divide_and_conquer<'scope>(scope: &Scope<'scope>, counter: &'scope AtomicUsize, size: usize) {
    if size > 1 {
        scope.spawn(move |scope| divide_and_conquer(scope, counter, size / 2));
        scope.spawn(move |scope| divide_and_conquer(scope, counter, size / 2));
    } else {
        // count the leaves
        counter.fetch_add(1, Ordering::SeqCst);
    }
}

fn divide_and_conquer_seq(counter: &AtomicUsize, size: usize) {
    if size > 1 {
        divide_and_conquer_seq(counter, size / 2);
        divide_and_conquer_seq(counter, size / 2);
    } else {
        // count the leaves
        counter.fetch_add(1, Ordering::SeqCst);
    }
}

struct Tree<T> {
    value: T,
    children: Vec<Tree<T>>,
}

impl<T> Tree<T> {
    pub fn iter<'s>(&'s self) -> impl Iterator<Item = &'s T> + 's {
        once(&self.value)
            .chain(self.children.iter().flat_map(|c| c.iter()))
            .collect::<Vec<_>>() // seems like it shouldn't be needed... but prevents overflow
            .into_iter()
    }

    pub fn update<OP>(&mut self, op: OP)
        where OP: Fn(&mut T) + Sync,
              T: Send
    {
        scope(|s| self.update_in_scope(&op, s));
    }

    fn update_in_scope<'scope, OP>(&'scope mut self, op: &'scope OP, scope: &Scope<'scope>)
        where OP: Fn(&mut T) + Sync
    {
        let Tree { ref mut value, ref mut children } = *self;
        scope.spawn(move |scope| {
            for child in children {
                scope.spawn(move |scope| child.update_in_scope(op, scope));
            }
        });

        op(value);
    }
}

fn random_tree(depth: usize) -> Tree<u32> {
    assert!(depth > 0);
    let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);
    random_tree1(depth, &mut rng)
}

fn random_tree1(depth: usize, rng: &mut XorShiftRng) -> Tree<u32> {
    let children = if depth == 0 {
        vec![]
    } else {
        (0..(rng.next_u32() % 3)) // somewhere between 0 and 3 children at each level
            .map(|_| random_tree1(depth - 1, rng))
            .collect()
    };

    Tree {
        value: rng.next_u32() % 1_000_000,
        children: children,
    }
}

#[test]
fn update_tree() {
    let mut tree: Tree<u32> = random_tree(10);
    let values: Vec<u32> = tree.iter().cloned().collect();
    tree.update(|v| *v += 1);
    let new_values: Vec<u32> = tree.iter().cloned().collect();
    assert_eq!(values.len(), new_values.len());
    for (&i, &j) in values.iter().zip(&new_values) {
        assert_eq!(i + 1, j);
    }
}

/// Check that if you have a chain of scoped tasks where T0 spawns T1
/// spawns T2 and so forth down to Tn, the stack space should not grow
/// linearly with N. We test this by some unsafe hackery and
/// permitting an approx 10% change with a 10x input change.
#[test]
fn linear_stack_growth() {
    let config = Configuration::new().num_threads(1);
    let pool = ThreadPool::new(config).unwrap();
    pool.install(|| {
        let mut max_diff = Mutex::new(0);
        let bottom_of_stack = 0;
        scope(|s| the_final_countdown(s, &bottom_of_stack, &max_diff, 5));
        let diff_when_5 = *max_diff.get_mut().unwrap() as f64;

        scope(|s| the_final_countdown(s, &bottom_of_stack, &max_diff, 500));
        let diff_when_500 = *max_diff.get_mut().unwrap() as f64;

        let ratio = diff_when_5 / diff_when_500;
        assert!(ratio > 0.9 && ratio < 1.1,
                "stack usage ratio out of bounds: {}",
                ratio);
    });
}

fn the_final_countdown<'scope>(s: &Scope<'scope>,
                               bottom_of_stack: &'scope i32,
                               max: &'scope Mutex<usize>,
                               n: usize) {
    let top_of_stack = 0;
    let p = bottom_of_stack as *const i32 as usize;
    let q = &top_of_stack as *const i32 as usize;
    let diff = if p > q { p - q } else { q - p };

    let mut data = max.lock().unwrap();
    *data = cmp::max(diff, *data);

    if n > 0 {
        s.spawn(move |s| the_final_countdown(s, bottom_of_stack, max, n - 1));
    }
}

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate_scope() {
    scope(|_| panic!("Hello, world!"));
}

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate_spawn() {
    scope(|s| s.spawn(|_| panic!("Hello, world!")));
}

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate_nested_spawn() {
    scope(|s| s.spawn(|s| s.spawn(|s| s.spawn(|_| panic!("Hello, world!")))));
}

#[test]
#[should_panic(expected = "Hello, world!")]
fn panic_propagate_nested_scope_spawn() {
    scope(|s| s.spawn(|_| scope(|s| s.spawn(|_| panic!("Hello, world!")))));
}

#[test]
fn panic_propagate_still_execute_1() {
    let mut x = false;
    match unwind::halt_unwinding(|| {
        scope(|s| {
            s.spawn(|_| panic!("Hello, world!")); // job A
            s.spawn(|_| x = true); // job B, should still execute even though A panics
        });
    }) {
        Ok(_) => panic!("failed to propagate panic"),
        Err(_) => assert!(x, "job b failed to execute"),
    }
}

#[test]
fn panic_propagate_still_execute_2() {
    let mut x = false;
    match unwind::halt_unwinding(|| {
        scope(|s| {
            s.spawn(|_| x = true); // job B, should still execute even though A panics
            s.spawn(|_| panic!("Hello, world!")); // job A
        });
    }) {
        Ok(_) => panic!("failed to propagate panic"),
        Err(_) => assert!(x, "job b failed to execute"),
    }
}

#[test]
fn panic_propagate_still_execute_3() {
    let mut x = false;
    match unwind::halt_unwinding(|| {
        scope(|s| {
            s.spawn(|_| x = true); // spanwed job should still execute despite later panic
            panic!("Hello, world!");
        });
    }) {
        Ok(_) => panic!("failed to propagate panic"),
        Err(_) => assert!(x, "panic after spawn, spawn failed to execute"),
    }
}

#[test]
fn panic_propagate_still_execute_4() {
    let mut x = false;
    match unwind::halt_unwinding(|| {
        scope(|s| {
            s.spawn(|_| panic!("Hello, world!"));
            x = true;
        });
    }) {
        Ok(_) => panic!("failed to propagate panic"),
        Err(_) => assert!(x, "panic in spawn tainted scope"),
    }
}
