use rayon;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::usize;

use super::graph::{Graph, Node};
use super::step;
use super::tour::TourPrefix;
use super::weight::Weight;

/// Shared context
pub struct SolverCx<'s> {
    graph: &'s Graph,
    seq_threshold: usize,
    priority_queue: Mutex<BinaryHeap<Arc<TourPrefix>>>,
    tour_counter: AtomicUsize,
    min_tour_weight: AtomicUsize,
    min_tour: Mutex<Option<Vec<Node>>>,
}

/// Just an opaque integer assigned to each tour element as we go;
/// lets us give them an ordering independent from the lower bound.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TourId {
    id: usize,
}

impl<'s> SolverCx<'s> {
    pub fn new(graph: &'s Graph, seq_threshold: usize) -> Self {
        SolverCx {
            graph: graph,
            seq_threshold: seq_threshold,
            priority_queue: Mutex::new(BinaryHeap::new()),
            tour_counter: AtomicUsize::new(0),
            min_tour_weight: AtomicUsize::new(usize::MAX),
            min_tour: Mutex::new(None),
        }
    }

    pub fn search_from(&mut self, node: Node) {
        // Enqueue the initial prefix:
        let id = self.tour_id();
        let mut visited = self.graph.node_set();
        visited.insert(node);
        self.priority_queue
            .get_mut()
            .unwrap()
            .push(
                Arc::new(
                    TourPrefix {
                        id: id,
                        node: node,
                        len: 1,
                        prefix_weight: Weight::zero(),
                        priority: Weight::max().to_priority(),
                        visited: visited,
                        previous: None,
                    },
                ),
            );

        // Start the iteration:
        rayon::scope(|s| step::step(s, self));
    }

    pub fn seq_threshold(&self) -> usize {
        self.seq_threshold
    }

    pub fn tour_id(&self) -> TourId {
        let counter = self.tour_counter.fetch_add(1, Ordering::SeqCst);
        TourId { id: counter }
    }

    pub fn graph(&self) -> &'s Graph {
        self.graph
    }

    pub fn enqueue(&self, tour_element: Arc<TourPrefix>) {
        let mut priority_queue = self.priority_queue.lock().unwrap();
        priority_queue.push(tour_element);
    }

    pub fn dequeue(&self) -> Option<Arc<TourPrefix>> {
        let mut priority_queue = self.priority_queue.lock().unwrap();
        let element = priority_queue.pop();
        element
    }

    pub fn min_tour_weight(&self) -> Weight {
        // Relaxed read is ok because the only time we *care* about
        // this being precise, we are holding `min_tour` lock; and
        // that is also the only time we write to it. This is subtle.
        Weight::new(self.min_tour_weight.load(Ordering::Relaxed))
    }

    pub fn add_complete_tour(&self, tour: &Vec<Node>, weight: Weight) {
        if weight < self.min_tour_weight() {
            let mut min_tour = self.min_tour.lock().unwrap();
            if min_tour.is_none() || weight < self.min_tour_weight() {
                // this is a new minimum!
                *min_tour = Some(tour.clone());
                self.min_tour_weight
                    .store(weight.to_usize(), Ordering::Relaxed);
            }
        }
    }

    pub fn into_result(self) -> (Option<Vec<Node>>, Weight) {
        let weight = self.min_tour_weight();
        (self.min_tour.into_inner().unwrap(), weight)
    }
}
