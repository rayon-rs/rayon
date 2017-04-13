use std::cmp::{PartialEq, Eq, PartialOrd, Ord, Ordering};
use std::sync::Arc;

use super::graph::{Node, NodeSet};
use super::weight::{Priority, Weight};
use super::solver::TourId;

#[derive(Clone, Debug)]
pub struct TourPrefix {
    /// priority to visit, derived from a lower bound on how much weight we
    /// have remaining to complete the tour
    pub priority: Priority,

    pub id: TourId,

    /// the next node to traverse
    pub node: Node,

    /// total length of our tour
    pub len: usize,

    /// total weight of our tour so far
    pub prefix_weight: Weight,

    /// bit set with elements left to visit
    pub visited: NodeSet,

    /// we extend this; this is ordered last so that the `Ord` impl
    /// won't look at it until the other fields
    pub previous: Option<Arc<TourPrefix>>,
}

impl TourPrefix {
    /// Returns a tuple of stuff to use when comparing for ord/eq
    fn to_cmp_elements(&self) -> (Priority, TourId) {
        (self.priority, self.id)
    }

    pub fn visited(&self, node: Node) -> bool {
        self.visited.contains(node)
    }
}

impl PartialEq for TourPrefix {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TourPrefix {}

impl PartialOrd for TourPrefix {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TourPrefix {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_cmp_elements().cmp(&other.to_cmp_elements())
    }
}
