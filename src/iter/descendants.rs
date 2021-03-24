use crate::iter::plumbing::{bridge_unindexed, Folder, UnindexedConsumer, UnindexedProducer};
use crate::prelude::*;
use std::collections::VecDeque;
use std::iter::once;
use std::marker::PhantomData;

struct DescendantsProducer<'b, S, B, I> {
    to_explore: VecDeque<Vec<S>>, // we do a depth first exploration so we need a stack
    seen: Vec<S>,                 // nodes which have already been explored
    breed: &'b B,                 // function generating children
    phantom: PhantomData<I>,
}

impl<'b, S, B, I> UnindexedProducer for DescendantsProducer<'b, S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    type Item = S;
    fn split(mut self) -> (Self, Option<Self>) {
        // explore while front is of size one.
        while self
            .to_explore
            .front()
            .map(|f| f.len() == 1)
            .unwrap_or(false)
        {
            let front_node = self.to_explore.pop_front().unwrap().pop().unwrap();
            let next_to_explore: Vec<_> = (self.breed)(&front_node).into_iter().collect();
            if !next_to_explore.is_empty() {
                self.to_explore.push_back(next_to_explore);
            }
            self.seen.push(front_node);
        }
        // now take half of the front.
        let f = self.to_explore.front_mut();
        let right_children = f.and_then(|f| split_vec(f));
        let right = right_children
            .map(|c| DescendantsProducer {
                to_explore: once(c).collect(),
                seen: Vec::new(),
                breed: self.breed,
                phantom: PhantomData,
            })
            .or_else(|| {
                // we can still try to divide 'seen'
                let right_seen = split_vec(&mut self.seen);
                right_seen.map(|s| DescendantsProducer {
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
        for mut e in self.to_explore {
            while let Some(s) = e.pop() {
                e.extend((self.breed)(&s));
                folder = folder.consume(s);
                if folder.full() {
                    return folder;
                }
            }
        }
        folder
    }
}

/// ParallelIterator for arbitrary tree-shaped patterns.
/// Returned by the [`descendants()`] function.
/// [`descendants()`]: fn.descendants.html
#[derive(Debug)]
pub struct Descendants<S, B, I> {
    initial_state: S,
    breed: B,
    phantom: PhantomData<I>,
}

impl<S, B, I> ParallelIterator for Descendants<S, B, I>
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
        let producer = DescendantsProducer {
            to_explore: once(vec![self.initial_state]).collect(),
            seen: Vec::new(),
            breed: &self.breed,
            phantom: PhantomData,
        };
        bridge_unindexed(producer, consumer)
    }
}

/// Divide given vector in two equally sized vectors.
/// Return `None` if initial size is <=1
fn split_vec<T>(v: &mut Vec<T>) -> Option<Vec<T>> {
    if v.len() <= 1 {
        None
    } else {
        let n = v.len() / 2;

        let mut left_vec = Vec::with_capacity(n);
        for _ in 0..n {
            left_vec.push(v.pop().unwrap());
        }
        Some(left_vec)
    }
}

/// Create a tree like parallel iterator from an initial root state.
/// Thre `breed` function should take a state and iterate on all of its children states.
///
/// # Example
///
/// ```
/// use rayon::prelude::*;
/// use rayon::iter::descendants;
/// assert_eq!(
///     descendants(4, |&e| if e <= 2 { Vec::new() } else {vec![e/2, e/2+1]})
///         .sum::<u32>(),
///     12);
/// ```
///
/// # Example
///
/// Here we loop on the following tree:
///                 4
///               /   \
///              /     \
///             2       3
///                    / \
///                   /   \
///                  1     2
///
/// The best parallelization is obtained when the tree is balanced
/// but we should also be able to handle harder cases.
///
///    ```
///    use rayon::prelude::*;
///    use rayon::iter::descendants;
///    struct Node {
///        content: u32,
///        left: Option<Box<Node>>,
///        right: Option<Box<Node>>,
///    }
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
///    let v: Vec<&Node> = descendants(&root, |r| {
///        r.left
///            .as_ref()
///            .into_iter()
///            .chain(r.right.as_ref())
///            .map(|n| &**n)
///    })
///    .collect();
///    assert_eq!(v.iter().filter(|n| n.content == 10).count(), 1);
///    assert_eq!(v.iter().filter(|n| n.content == 18).count(), 1);
///    assert_eq!(v.len(), 4);
///    ```
///
pub fn descendants<S, B, I>(root: S, breed: B) -> Descendants<S, B, I>
where
    S: Send,
    B: Fn(&S) -> I + Send + Sync,
    I: IntoIterator<Item = S> + Send,
{
    Descendants {
        initial_state: root,
        breed,
        phantom: PhantomData,
    }
}
