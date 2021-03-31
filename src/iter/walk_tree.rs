use crate::iter::plumbing::{bridge_unindexed, Folder, UnindexedConsumer, UnindexedProducer};
use crate::prelude::*;
use std::collections::VecDeque;
use std::iter::once;
use std::marker::PhantomData;

#[derive(Debug)]
struct WalkTreePrefixProducer<'b, S, B, I> {
    to_explore: VecDeque<S>, // nodes (and subtrees) we have to process
    seen: Vec<S>,            // nodes which have already been explored
    breed: &'b B,            // function generating children
    phantom: PhantomData<I>,
}

impl<'b, S, B, I> UnindexedProducer for WalkTreePrefixProducer<'b, S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    type Item = S;
    fn split(mut self) -> (Self, Option<Self>) {
        // explore while front is of size one.
        while self.to_explore.len() == 1 {
            let front_node = self.to_explore.pop_front().unwrap();
            self.to_explore = (self.breed)(&front_node).into_iter().collect();
            self.seen.push(front_node);
        }
        // now take half of the front.
        let right_children = split_vecdeque(&mut self.to_explore);
        let right = right_children
            .map(|c| WalkTreePrefixProducer {
                to_explore: c,
                seen: Vec::new(),
                breed: self.breed,
                phantom: PhantomData,
            })
            .or_else(|| {
                // we can still try to divide 'seen'
                let right_seen = split_vec(&mut self.seen);
                right_seen.map(|s| WalkTreePrefixProducer {
                    to_explore: Default::default(),
                    seen: s,
                    breed: self.breed,
                    phantom: PhantomData,
                })
            });
        (self, right)
    }
    fn fold_with<F>(self, mut folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        // start by consuming everything seen
        for s in self.seen {
            folder = folder.consume(s);
            if folder.full() {
                return folder;
            }
        }
        // now do all remaining explorations
        for e in self.to_explore {
            folder = consume_rec_prefix(&self.breed, e, folder);
            if folder.full() {
                return folder;
            }
        }
        folder
    }
}

fn consume_rec_prefix<F: Folder<S>, S, B: Fn(&S) -> I, I: IntoIterator<Item = S>>(
    breed: &B,
    s: S,
    mut folder: F,
) -> F {
    let children = (breed)(&s).into_iter().collect::<Vec<_>>();
    folder = folder.consume(s);
    if folder.full() {
        return folder;
    }
    for child in children {
        folder = consume_rec_prefix(breed, child, folder);
        if folder.full() {
            return folder;
        }
    }
    folder
}

/// ParallelIterator for arbitrary tree-shaped patterns.
/// Returned by the [`walk_tree_prefix()`] function.
/// [`walk_tree_prefix()`]: fn.walk_tree_prefix.html
#[derive(Debug)]
pub struct WalkTreePrefix<S, B, I> {
    initial_state: S,
    breed: B,
    phantom: PhantomData<I>,
}

impl<S, B, I> ParallelIterator for WalkTreePrefix<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    type Item = S;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let producer = WalkTreePrefixProducer {
            to_explore: once(self.initial_state).collect(),
            seen: Vec::new(),
            breed: &self.breed,
            phantom: PhantomData,
        };
        bridge_unindexed(producer, consumer)
    }
}

/// Create a tree like parallel iterator from an initial root state.
/// Thre `breed` function should take a state and iterate on all of its children states.
/// The best parallelization is obtained when the tree is balanced
/// but we should also be able to handle harder cases.
///
/// This guarantees a prefix ordering. For a postfix ordering see the (faster) [`walk_tree_postfix()`] function.
///
/// [`walk_tree_postfix()`]: fn.walk_tree_postfix.html
///
/// # Example
///
/// ```
/// use rayon::prelude::*;
/// use rayon::iter::walk_tree_prefix;
/// assert_eq!(
///     walk_tree_prefix(4, |&e| if e <= 2 { Vec::new() } else {vec![e/2, e/2+1]})
///         .sum::<u32>(),
///     12);
/// ```
///
/// # Example
///
///    ```
///    use rayon::prelude::*;
///    use rayon::iter::walk_tree_prefix;
///
///    struct Node {
///        content: u32,
///        left: Option<Box<Node>>,
///        right: Option<Box<Node>>,
///    }
///
/// // Here we loop on the following tree:
/// //
/// //       10
/// //      /  \
/// //     /    \
/// //    3     14
/// //            \
/// //             \
/// //              18
///
///    let root = Node {
///        content: 10,
///        left: Some(Box::new(Node {
///            content: 3,
///            left: None,
///            right: None,
///        })),
///        right: Some(Box::new(Node {
///            content: 14,
///            left: None,
///            right: Some(Box::new(Node {
///                content: 18,
///                left: None,
///                right: None,
///            })),
///        })),
///    };
///    let mut v: Vec<u32> = walk_tree_prefix(&root, |r| {
///        r.left
///            .as_ref()
///            .into_iter()
///            .chain(r.right.as_ref())
///            .map(|n| &**n)
///    })
///    .map(|node| node.content)
///    .collect();
///    assert_eq!(v, vec![10, 3, 14, 18]);
///    ```
///
pub fn walk_tree_prefix<S, B, I>(root: S, breed: B) -> WalkTreePrefix<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    WalkTreePrefix {
        initial_state: root,
        breed,
        phantom: PhantomData,
    }
}

// post fix

#[derive(Debug)]
struct WalkTreePostfixProducer<'b, S, B, I> {
    to_explore: VecDeque<S>, // nodes (and subtrees) we have to process
    seen: Vec<S>,            // nodes which have already been explored
    breed: &'b B,            // function generating children
    phantom: PhantomData<I>,
}

impl<'b, S, B, I> UnindexedProducer for WalkTreePostfixProducer<'b, S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    type Item = S;
    fn split(mut self) -> (Self, Option<Self>) {
        // explore while front is of size one.
        while self.to_explore.len() == 1 {
            let front_node = self.to_explore.pop_front().unwrap();
            self.to_explore = (self.breed)(&front_node).into_iter().collect();
            self.seen.push(front_node);
        }
        // now take half of the front.
        let right_children = split_vecdeque(&mut self.to_explore);
        let right = right_children
            .map(|c| {
                let mut right_seen = Vec::new();
                std::mem::swap(&mut self.seen, &mut right_seen); // postfix -> upper nodes are processed last
                WalkTreePostfixProducer {
                    to_explore: c,
                    seen: right_seen,
                    breed: self.breed,
                    phantom: PhantomData,
                }
            })
            .or_else(|| {
                // we can still try to divide 'seen'
                let right_seen = split_vec(&mut self.seen);
                right_seen.map(|mut s| {
                    std::mem::swap(&mut self.seen, &mut s);
                    WalkTreePostfixProducer {
                        to_explore: Default::default(),
                        seen: s,
                        breed: self.breed,
                        phantom: PhantomData,
                    }
                })
            });
        (self, right)
    }
    fn fold_with<F>(self, mut folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        // now do all remaining explorations
        for e in self.to_explore {
            folder = consume_rec_postfix(&self.breed, e, folder);
            if folder.full() {
                return folder;
            }
        }
        // end by consuming everything seen
        for s in self.seen.into_iter().rev() {
            folder = folder.consume(s);
            if folder.full() {
                return folder;
            }
        }
        folder
    }
}

fn consume_rec_postfix<F: Folder<S>, S, B: Fn(&S) -> I, I: IntoIterator<Item = S>>(
    breed: &B,
    s: S,
    mut folder: F,
) -> F {
    let children = (breed)(&s).into_iter();
    for child in children {
        folder = consume_rec_postfix(breed, child, folder);
        if folder.full() {
            return folder;
        }
    }
    folder.consume(s)
}

/// ParallelIterator for arbitrary tree-shaped patterns.
/// Returned by the [`walk_tree_postfix()`] function.
/// [`walk_tree_postfix()`]: fn.walk_tree_postfix.html
#[derive(Debug)]
pub struct WalkTreePostfix<S, B, I> {
    initial_state: S,
    breed: B,
    phantom: PhantomData<I>,
}

impl<S, B, I> ParallelIterator for WalkTreePostfix<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    type Item = S;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let producer = WalkTreePostfixProducer {
            to_explore: once(self.initial_state).collect(),
            seen: Vec::new(),
            breed: &self.breed,
            phantom: PhantomData,
        };
        bridge_unindexed(producer, consumer)
    }
}

/// Divide given deque in two equally sized deques.
/// Return `None` if initial size is <=1.
/// We return the first half and keep the last half in `v`.
fn split_vecdeque<T>(v: &mut VecDeque<T>) -> Option<VecDeque<T>> {
    if v.len() <= 1 {
        None
    } else {
        let n = v.len() / 2;
        Some(v.split_off(n))
    }
}

/// Divide given vector in two equally sized vectors.
/// Return `None` if initial size is <=1.
/// We return the first half and keep the last half in `v`.
fn split_vec<T>(v: &mut Vec<T>) -> Option<Vec<T>> {
    if v.len() <= 1 {
        None
    } else {
        let n = v.len() / 2;
        Some(v.split_off(n))
    }
}

/// Create a tree like parallel iterator from an initial root state.
/// Thre `breed` function should take a state and iterate on all of its children states.
/// The best parallelization is obtained when the tree is balanced
/// but we should also be able to handle harder cases.
///
/// This guarantees a postfix ordering. For a prefix ordering see the (slower) [`walk_tree_prefix()`] function.
///
/// [`walk_tree_prefix()`]: fn.walk_tree_prefix.html
///
/// # Example
///
/// ```
/// use rayon::prelude::*;
/// use rayon::iter::walk_tree_postfix;
/// assert_eq!(
///     walk_tree_postfix(4, |&e| if e <= 2 { Vec::new() } else {vec![e/2, e/2+1]})
///         .sum::<u32>(),
///     12);
/// ```
///
/// # Example
///
///    ```
///    use rayon::prelude::*;
///    use rayon::iter::walk_tree_postfix;
///
///    struct Node {
///        content: u32,
///        left: Option<Box<Node>>,
///        right: Option<Box<Node>>,
///    }
///
/// // Here we loop on the following tree:
/// //
/// //       10
/// //      /  \
/// //     /    \
/// //    3     14
/// //            \
/// //             \
/// //              18
///
///    let root = Node {
///        content: 10,
///        left: Some(Box::new(Node {
///            content: 3,
///            left: None,
///            right: None,
///        })),
///        right: Some(Box::new(Node {
///            content: 14,
///            left: None,
///            right: Some(Box::new(Node {
///                content: 18,
///                left: None,
///                right: None,
///            })),
///        })),
///    };
///    let mut v: Vec<u32> = walk_tree_postfix(&root, |r| {
///        r.left
///            .as_ref()
///            .into_iter()
///            .chain(r.right.as_ref())
///            .map(|n| &**n)
///    })
///    .map(|node| node.content)
///    .collect();
///    assert_eq!(v, vec![3, 18, 14, 10]);
///    ```
///
pub fn walk_tree_postfix<S, B, I>(root: S, breed: B) -> WalkTreePostfix<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    WalkTreePostfix {
        initial_state: root,
        breed,
        phantom: PhantomData,
    }
}
