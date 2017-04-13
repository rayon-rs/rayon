use fixedbitset::FixedBitSet;
use std::iter;

use super::weight::Weight;

pub struct Graph {
    num_nodes: usize,

    // a 2-d matrix indexed by (source, target); if `Weight::max()`
    // is stored in a particular entry, that means that there is no
    // edge. Otherwise, the weight is found.
    weights: Vec<Weight>,
}

impl Graph {
    pub fn new(num_nodes: usize) -> Graph {
        Graph {
            num_nodes: num_nodes,
            weights: iter::repeat(Weight::max())
                .take(num_nodes * num_nodes)
                .collect(),
        }
    }

    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    pub fn all_nodes(&self) -> impl Iterator<Item = Node> {
        (0..self.num_nodes).map(Node::new)
    }

    pub fn node_set(&self) -> NodeSet {
        NodeSet { bits: FixedBitSet::with_capacity(self.num_nodes) }
    }

    fn edge_index(&self, source: Node, target: Node) -> usize {
        (source.index * self.num_nodes) + target.index
    }

    pub fn set_weight(&mut self, source: Node, target: Node, w: Weight) {
        assert!(!w.is_max());
        let index = self.edge_index(source, target);
        self.weights[index] = w;
    }

    pub fn edge_weight(&self, source: Node, target: Node) -> Option<Weight> {
        let w = self.weights[self.edge_index(source, target)];
        if w.is_max() { None } else { Some(w) }
    }

    pub fn edges<'a>(&'a self, source: Node) -> impl Iterator<Item = Edge> {
        self.all_nodes()
            .filter_map(
                move |target| {
                    self.edge_weight(source, target)
                        .map(
                            |weight| {
                                Edge {
                                    source: source,
                                    target: target,
                                    weight: weight,
                                }
                            },
                        )
                },
            )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Node {
    index: usize,
}

impl Node {
    pub fn new(index: usize) -> Node {
        Node { index: index }
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Edge {
    pub source: Node,
    pub target: Node,
    pub weight: Weight,
}

#[derive(Clone, Debug)]
pub struct NodeSet {
    bits: FixedBitSet,
}

impl NodeSet {
    pub fn contains(&self, node: Node) -> bool {
        self.bits.contains(node.index)
    }

    pub fn with(&self, node: Node) -> NodeSet {
        let mut s = self.clone();
        s.insert(node);
        s
    }

    pub fn insert(&mut self, node: Node) {
        self.bits.set(node.index, true);
    }

    pub fn remove(&mut self, node: Node) {
        self.bits.set(node.index, false);
    }
}
