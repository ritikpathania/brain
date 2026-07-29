//! Reusable, workflow-agnostic Directed Acyclic Graph (DAG) for reflection tasks.

use crate::reflection::contracts::ReflectionTask;
use std::collections::{HashMap, VecDeque};

/// A node in the `TaskDag` containing a task and its declared prerequisite task IDs.
pub struct TaskNode {
    /// Unique task identifier within the DAG.
    pub id: String,
    /// Boxed reflection task instance.
    pub task: Box<dyn ReflectionTask>,
    /// IDs of prerequisite tasks that must complete before this node executes.
    pub dependencies: Vec<String>,
}

/// Generic Directed Acyclic Graph (DAG) managing task dependencies and topological stage levelization.
#[derive(Default)]
pub struct TaskDag {
    nodes: HashMap<String, TaskNode>,
}

impl TaskDag {
    /// Creates a new empty `TaskDag`.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Adds a task node with its prerequisite dependency IDs to the DAG.
    pub fn add_node<T: ReflectionTask + 'static>(
        &mut self,
        id: impl Into<String>,
        task: T,
        dependencies: Vec<String>,
    ) {
        let node_id = id.into();
        self.nodes.insert(
            node_id.clone(),
            TaskNode {
                id: node_id,
                task: Box::new(task),
                dependencies,
            },
        );
    }

    /// Returns a reference to a node by ID.
    pub fn get_node(&self, id: &str) -> Option<&TaskNode> {
        self.nodes.get(id)
    }

    /// Returns the total node count in the DAG.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true if the DAG contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Computes topological execution stages (levelization).
    ///
    /// Tasks in the same stage have all dependencies satisfied by prior stages
    /// and can be safely executed concurrently or sequentially.
    ///
    /// Returns an error if a dependency cycle is detected or if a dependency ID is missing.
    pub fn compute_stages(&self) -> Result<Vec<Vec<String>>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for (id, node) in &self.nodes {
            in_degree.entry(id.clone()).or_insert(0);
            for dep in &node.dependencies {
                if !self.nodes.contains_key(dep) {
                    return Err(format!(
                        "Task '{}' relies on missing dependency '{}'",
                        id, dep
                    ));
                }
                dependents.entry(dep.clone()).or_default().push(id.clone());
                *in_degree.entry(id.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut stages: Vec<Vec<String>> = Vec::new();
        let mut processed_count = 0;

        while !queue.is_empty() {
            let stage_size = queue.len();
            let mut current_stage = Vec::with_capacity(stage_size);

            for _ in 0..stage_size {
                let node_id = queue.pop_front().expect("Queue must not be empty");
                processed_count += 1;
                current_stage.push(node_id.clone());

                if let Some(children) = dependents.get(&node_id) {
                    for child in children {
                        let deg = in_degree.get_mut(child).expect("In-degree must exist");
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(child.clone());
                        }
                    }
                }
            }

            // Sort stage task IDs deterministically
            current_stage.sort();
            stages.push(current_stage);
        }

        if processed_count != self.nodes.len() {
            return Err("Dependency cycle detected in TaskDag".to_string());
        }

        Ok(stages)
    }
}
