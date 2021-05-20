use crate::iter::plumbing::{bridge_unindexed, Folder, UnindexedConsumer, UnindexedProducer};
use crate::prelude::*;
use std::iter::once;
use std::marker::PhantomData;

#[derive(Debug)]
struct WalkTreePrefixProducer<'b, S, B, I> {
    to_explore: Vec<S>, // nodes (and subtrees) we have to process
    seen: Vec<S>,       // nodes which have already been explored
    children_of: &'b B, // function generating children
    phantom: PhantomData<I>,
}

impl<'b, S, B, I, IT> UnindexedProducer for WalkTreePrefixProducer<'b, S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    IT: DoubleEndedIterator<Item = S>,
    I: IntoIterator<Item = S, IntoIter = IT> + Send,
{
    type Item = S;
    fn split(mut self) -> (Self, Option<Self>) {
        // explore while front is of size one.
        while self.to_explore.len() == 1 {
            let front_node = self.to_explore.pop().unwrap();
            self.to_explore
                .extend((self.children_of)(&front_node).into_iter().rev());
            self.seen.push(front_node);
        }
        // now take half of the front.
        let right_children = split_vec(&mut self.to_explore);
        let right = right_children
            .map(|mut c| {
                std::mem::swap(&mut c, &mut self.to_explore);
                WalkTreePrefixProducer {
                    to_explore: c,
                    seen: Vec::new(),
                    children_of: self.children_of,
                    phantom: PhantomData,
                }
            })
            .or_else(|| {
                // we can still try to divide 'seen'
                let right_seen = split_vec(&mut self.seen);
                right_seen.map(|s| WalkTreePrefixProducer {
                    to_explore: Default::default(),
                    seen: s,
                    children_of: self.children_of,
                    phantom: PhantomData,
                })
            });
        (self, right)
    }
    fn fold_with<F>(mut self, mut folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        // start by consuming everything seen
        folder = folder.consume_iter(self.seen);
        if folder.full() {
            return folder;
        }
        // now do all remaining explorations
        while let Some(e) = self.to_explore.pop() {
            self.to_explore
                .extend((self.children_of)(&e).into_iter().rev());
            folder = folder.consume(e);
            if folder.full() {
                return folder;
            }
        }
        folder
    }
}

/// ParallelIterator for arbitrary tree-shaped patterns.
/// Returned by the [`walk_tree_prefix()`] function.
///
/// [`walk_tree_prefix()`]: fn.walk_tree_prefix.html
#[derive(Debug)]
pub struct WalkTreePrefix<S, B, I> {
    initial_state: S,
    children_of: B,
    phantom: PhantomData<I>,
}

impl<S, B, I, IT> ParallelIterator for WalkTreePrefix<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    IT: DoubleEndedIterator<Item = S>,
    I: IntoIterator<Item = S, IntoIter = IT> + Send,
{
    type Item = S;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let producer = WalkTreePrefixProducer {
            to_explore: once(self.initial_state).collect(),
            seen: Vec::new(),
            children_of: &self.children_of,
            phantom: PhantomData,
        };
        bridge_unindexed(producer, consumer)
    }
}

/// Create a tree-like prefix parallel iterator from an initial root node.
/// The `children_of` function should take a node and return an iterator over its child nodes.
/// The best parallelization is obtained when the tree is balanced
/// but we should also be able to handle harder cases.
///
/// # Ordering
///
/// This function guarantees a prefix ordering. See also [`walk_tree_postfix`],
/// which guarantees a postfix order.
/// If you don't care about ordering, you should use [`walk_tree`],
/// which will use whatever is believed to be fastest.
/// For example a perfect binary tree of 7 nodes will reduced in the following order:
///
/// ```text
///      a
///     / \
///    /   \
///   b     c
///  / \   / \
/// d   e f   g
///
/// reduced as  a,b,d,e,c,f,g
///
/// ```
///
///
/// For a postfix ordering see the (faster) [`walk_tree_postfix()`] function.
///
/// [`walk_tree_postfix()`]: fn.walk_tree_postfix.html
/// [`walk_tree()`]: fn.walk_tree.html
///
/// # Example
///
/// ```text
///      4
///     / \
///    /   \
///   2     3
///        / \
///       1   2
/// ```
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
pub fn walk_tree_prefix<S, B, I>(root: S, children_of: B) -> WalkTreePrefix<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I::IntoIter: DoubleEndedIterator<Item = S>,
    I: IntoIterator<Item = S> + Send,
{
    WalkTreePrefix {
        initial_state: root,
        children_of,
        phantom: PhantomData,
    }
}

// post fix

#[derive(Debug)]
struct WalkTreePostfixProducer<'b, S, B, I> {
    to_explore: Vec<S>, // nodes (and subtrees) we have to process
    seen: Vec<S>,       // nodes which have already been explored
    children_of: &'b B, // function generating children
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
            let front_node = self.to_explore.pop().unwrap();
            self.to_explore
                .extend((self.children_of)(&front_node).into_iter());
            self.seen.push(front_node);
        }
        // now take half of the front.
        let right_children = split_vec(&mut self.to_explore);
        let right = right_children
            .map(|c| {
                let mut right_seen = Vec::new();
                std::mem::swap(&mut self.seen, &mut right_seen); // postfix -> upper nodes are processed last
                WalkTreePostfixProducer {
                    to_explore: c,
                    seen: right_seen,
                    children_of: self.children_of,
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
                        children_of: self.children_of,
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
            folder = consume_rec_postfix(&self.children_of, e, folder);
            if folder.full() {
                return folder;
            }
        }
        // end by consuming everything seen
        folder.consume_iter(self.seen.into_iter().rev())
    }
}

fn consume_rec_postfix<F: Folder<S>, S, B: Fn(&S) -> I, I: IntoIterator<Item = S>>(
    children_of: &B,
    s: S,
    mut folder: F,
) -> F {
    let children = (children_of)(&s).into_iter();
    for child in children {
        folder = consume_rec_postfix(children_of, child, folder);
        if folder.full() {
            return folder;
        }
    }
    folder.consume(s)
}

/// ParallelIterator for arbitrary tree-shaped patterns.
/// Returned by the [`walk_tree_postfix()`] function.
///
/// [`walk_tree_postfix()`]: fn.walk_tree_postfix.html
#[derive(Debug)]
pub struct WalkTreePostfix<S, B, I> {
    initial_state: S,
    children_of: B,
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
            children_of: &self.children_of,
            phantom: PhantomData,
        };
        bridge_unindexed(producer, consumer)
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

/// Create a tree like postfix parallel iterator from an initial root node.
/// The `children_of` function should take a node and iterate on all of its child nodes.
/// The best parallelization is obtained when the tree is balanced
/// but we should also be able to handle harder cases.
///
/// # Ordering
///
/// This function guarantees a postfix ordering. See also [`walk_tree_prefix`] which guarantees a
/// prefix order. If you don't care about ordering, you should use [`walk_tree`], which will use
/// whatever is believed to be fastest.
///
/// Between siblings, children are reduced in order -- that is first children are reduced first.
///
/// For example a perfect binary tree of 7 nodes will reduced in the following order:
///
/// ```text
///      a
///     / \
///    /   \
///   b     c
///  / \   / \
/// d   e f   g
///
/// reduced as d,e,b,f,g,c,a
///
/// ```
///
/// For a prefix ordering see the (slower) [`walk_tree_prefix()`] function.
///
/// [`walk_tree_prefix()`]: fn.walk_tree_prefix.html
/// [`walk_tree()`]: fn.walk_tree.html
///
/// # Example
///
/// ```text
///      4
///     / \
///    /   \
///   2     3
///        / \
///       1   2
/// ```
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
pub fn walk_tree_postfix<S, B, I>(root: S, children_of: B) -> WalkTreePostfix<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    WalkTreePostfix {
        initial_state: root,
        children_of,
        phantom: PhantomData,
    }
}

/// ParallelIterator for arbitrary tree-shaped patterns.
/// Returned by the [`walk_tree()`] function.
///
/// [`walk_tree()`]: fn.walk_tree_prefix.html
#[derive(Debug)]
pub struct WalkTree<S, B, I>(WalkTreePostfix<S, B, I>);

/// Create a tree like parallel iterator from an initial root node.
/// The `children_of` function should take a node and iterate on all of its child nodes.
/// The best parallelization is obtained when the tree is balanced
/// but we should also be able to handle harder cases.
///
/// # Ordering
///
/// This function does not guarantee any ordering but will
/// use whatever algorithm is thought to achieve the fastest traversal.
/// See also [`walk_tree_prefix`] which guarantees a
/// prefix order and [`walk_tree_postfix`] which guarantees a postfix order.
///
/// # Example
///
/// ```text
///      4
///     / \
///    /   \
///   2     3
///        / \
///       1   2
/// ```
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
/// [`walk_tree_prefix()`]: fn.walk_tree_prefix.html
/// [`walk_tree_postfix()`]: fn.walk_tree_postfix.html
pub fn walk_tree<S, B, I>(root: S, children_of: B) -> WalkTree<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    let walker = WalkTreePostfix {
        initial_state: root,
        children_of,
        phantom: PhantomData,
    };
    WalkTree(walker)
}

impl<S, B, I> ParallelIterator for WalkTree<S, B, I>
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
        self.0.drive_unindexed(consumer)
    }
}
