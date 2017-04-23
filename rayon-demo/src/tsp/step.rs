use rayon::Scope;
use std::cmp;
use std::sync::Arc;

use super::graph::{Graph, Node, NodeSet};
use super::tour::TourPrefix;
use super::solver::SolverCx;
use super::weight::Weight;

pub fn step<'s>(scope: &Scope<'s>, solver: &'s SolverCx<'s>) {
    let element = solver.dequeue().unwrap();

    let remaining = solver.graph().num_nodes() - element.len;
    if remaining <= solver.seq_threshold() {
        solve_tour_seq(solver, element);
    } else {
        split_tour(scope, solver, element);
    }
}

fn split_tour<'s>(scope: &Scope<'s>, solver: &'s SolverCx<'s>, element: Arc<TourPrefix>) {
    let graph = solver.graph();
    let last_node = element.node;
    for next_edge in graph.edges(last_node) {
        // for each place we could go next...
        let next_node = next_edge.target;
        if !element.visited(next_node) {
            // ...check we haven't already been there...
            let weight = next_edge.weight;
            if (element.prefix_weight + weight) < solver.min_tour_weight() {
                // ...and that we haven't already found a cheaper route
                let visited = element.visited.with(next_node);

                let prefix_weight = element.prefix_weight + weight;

                let next_lower_bound =
                    compute_lower_bound(solver.graph(), &element.visited, &visited, prefix_weight);

                let next_tour = Arc::new(
                    TourPrefix {
                        id: solver.tour_id(),
                        priority: next_lower_bound.to_priority(),
                        node: next_node,
                        len: element.len + 1,
                        prefix_weight: prefix_weight,
                        visited: visited,
                        previous: Some(element.clone()),
                    },
                );
                solver.enqueue(next_tour);
                scope.spawn(move |s| step(s, solver));
            }
        }
    }
}

/// Compute a lower bound for completing the tour, given that we have
/// a partial tour which visits the nodes in `visited` at the cost of
/// `weight`. There are a number of ways you could to this; this one
/// is intended to be a general method. We look at each node that is
/// not yet visited and find the cheapest incoming edge and sum those
/// up.  The idea is that we must *somehow* get to each of those nodes.
fn compute_lower_bound(
    graph: &Graph,
    prev_visited: &NodeSet,
    visited: &NodeSet,
    weight: Weight,
) -> Weight {
    // Our path looks like this
    //
    //     START ~> n0 ... nJ ~> nK ~> ... nZ -> START
    //     ------------------    --    ---------------
    //     prev_visited       next_node     the future
    //
    // We want to find all edges that are targeting "the future"
    // from a node that is *not* in `prev_visited`.

    let mut min_weights: Vec<_> = graph
        .all_nodes()
        .map(
            |i| if visited.contains(i) {
                Weight::zero()
            } else {
                Weight::max()
            },
        )
        .collect();

    for i in graph.all_nodes().filter(|&i| !prev_visited.contains(i)) {
        for j in graph.all_nodes().filter(|&j| !visited.contains(j)) {
            if let Some(w) = graph.edge_weight(i, j) {
                // track the cheapest way to reach node `j` that doesn't
                // start from one of the nodes we've been to already (but
                // maybe starts from the most recent node):
                min_weights[j.index()] = cmp::min(w, min_weights[j.index()]);
            }
        }
    }

    min_weights
        .iter()
        .fold(Weight::zero(), |w1, &w2| w1 + w2) + weight
}

fn solve_tour_seq(solver: &SolverCx, element: Arc<TourPrefix>) {
    // Sequentially enumerate all possible tours starting from this point.
    let graph = solver.graph();
    let mut path = Vec::with_capacity(graph.num_nodes() + 1);
    let mut visited = element.visited.clone();

    // unwind the current prefix into `path`; the path is stored in
    // reverse order, so we have to reverse at the end. i.e., if the
    // path were N0, N1, N2, we would have:
    //
    //     N2 -> N1 -> N0
    let mut p = &element;
    loop {
        path.push(p.node);
        if let Some(ref n) = p.previous {
            p = n;
        } else {
            break;
        }
    }
    path.reverse();

    if path.len() == graph.num_nodes() {
        complete_tour(solver, &mut path, element.prefix_weight);
    } else {
        enumerate_sequentially(solver, &mut path, &mut visited, element.prefix_weight);
    }
}

fn enumerate_sequentially(
    solver: &SolverCx,
    path: &mut Vec<Node>,
    visited: &mut NodeSet,
    mut weight: Weight,
) {
    // Try to figure out what node to visit next.
    let graph = solver.graph();
    let prev = *path.last().unwrap();
    for i in graph.all_nodes() {
        // Don't go back places we've already been.
        if visited.contains(i) {
            continue;
        }

        // Not connected to previous node.
        let edge_weight = match graph.edge_weight(prev, i) {
            Some(w) => w,
            None => continue,
        };

        // Check if this is better than cheapest found by anybody else so far.
        if weight + edge_weight > solver.min_tour_weight() {
            continue;
        }

        // Commit to `i` as the next node for us to visit.
        weight += edge_weight;
        path.push(i);
        visited.insert(i);

        if path.len() < graph.num_nodes() {
            // Not yet completed the whole graph, keep looking.
            enumerate_sequentially(solver, path, visited, weight);
        } else {
            // Completed the whole graph; we have to get back to the start node now (if we can).
            complete_tour(solver, path, weight);
        }

        // Uncommit to `i`.
        weight -= edge_weight;
        path.pop();
        visited.remove(i);
    }
}

fn complete_tour(solver: &SolverCx, path: &mut Vec<Node>, weight: Weight) {
    let graph = solver.graph();
    debug_assert!(path.len() == graph.num_nodes());
    let home = path[0];
    let last = *path.last().unwrap();
    if let Some(home_weight) = graph.edge_weight(last, home) {
        path.push(home);
        solver.add_complete_tour(&path, weight + home_weight);
        path.pop();
    }
}
