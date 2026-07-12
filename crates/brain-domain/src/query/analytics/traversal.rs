use crate::identifiers::NodeId;
use crate::query::analytics::GraphAnalyticsContext;
use std::collections::{HashSet, VecDeque};

/// Traversal utility helpers for graph exploration.
pub struct GraphTraversal;

impl GraphTraversal {
    /// Performs a Depth-First Search (DFS) starting from `start` node and calls `visitor` callback for each visited node.
    pub fn dfs<F>(context: &GraphAnalyticsContext, start: NodeId, visited: &mut HashSet<NodeId>, mut visitor: F)
    where
        F: FnMut(NodeId),
    {
        let mut stack = Vec::new();
        stack.push(start);

        let adjacency = context.adjacency();

        while let Some(node) = stack.pop() {
            if visited.insert(node) {
                visitor(node);
                for &neighbor in adjacency.neighbors(node).iter().rev() {
                    if !visited.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }
    }

    /// Performs a Breadth-First Search (BFS) starting from `start` node and calls `visitor` callback for each visited node.
    pub fn bfs<F>(context: &GraphAnalyticsContext, start: NodeId, visited: &mut HashSet<NodeId>, mut visitor: F)
    where
        F: FnMut(NodeId),
    {
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited.insert(start);

        let adjacency = context.adjacency();

        while let Some(node) = queue.pop_front() {
            visitor(node);
            for &neighbor in adjacency.neighbors(node) {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
    }
}
