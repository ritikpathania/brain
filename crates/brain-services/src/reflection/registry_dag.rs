//! Reflection v2 pass registry and topological DAG execution resolver.

use crate::reflection::pass_context::*;
use brain_domain::bkf::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Registry managing registered Reflection Engine v2 passes and resolving topological DAG execution order.
#[derive(Default)]
pub struct PassRegistryV2 {
    passes: HashMap<PassId, Arc<dyn V2ReflectionPass>>,
}

impl PassRegistryV2 {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            passes: HashMap::new(),
        }
    }

    /// Registers a new reflection pass. Errors if a pass with the same `PassId` is already registered.
    pub fn register(&mut self, pass: Box<dyn V2ReflectionPass>) -> Result<(), String> {
        let id = pass.id();
        if self.passes.contains_key(&id) {
            return Err(format!("Pass with ID '{}' is already registered", id.as_str()));
        }
        self.passes.insert(id, Arc::from(pass));
        Ok(())
    }

    /// Resolves and returns registered passes sorted in deterministic topological DAG dependency order.
    /// Returns error if missing dependencies or cyclic dependencies are detected.
    pub fn resolve_execution_order(&self) -> Result<Vec<Arc<dyn V2ReflectionPass>>, String> {
        // 1. Verify all declared dependencies exist
        for (id, pass) in &self.passes {
            for dep in pass.dependencies() {
                if !self.passes.contains_key(dep) {
                    return Err(format!(
                        "Pass '{}' requires missing dependency '{}'",
                        id.as_str(),
                        dep.as_str()
                    ));
                }
            }
        }

        // 2. Build in-degree counts and adjacency graph (dep -> dependent)
        let mut in_degree: HashMap<PassId, usize> = HashMap::new();
        let mut adj: HashMap<PassId, Vec<PassId>> = HashMap::new();

        for id in self.passes.keys() {
            in_degree.insert(id.clone(), 0);
            adj.insert(id.clone(), Vec::new());
        }

        for (id, pass) in &self.passes {
            for dep in pass.dependencies() {
                adj.get_mut(dep).unwrap().push(id.clone());
                *in_degree.get_mut(id).unwrap() += 1;
            }
        }

        // 3. Kahn's algorithm with deterministic tiebreaker sorting by PassId
        let mut ready: Vec<PassId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        ready.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        let mut queue: VecDeque<PassId> = ready.into();
        let mut sorted_ids: Vec<PassId> = Vec::new();
        let mut visited: HashSet<PassId> = HashSet::new();

        while let Some(curr) = queue.pop_front() {
            sorted_ids.push(curr.clone());
            visited.insert(curr.clone());

            let mut newly_ready: Vec<PassId> = Vec::new();
            if let Some(dependents) = adj.get(&curr) {
                for dep in dependents {
                    let deg = in_degree.get_mut(dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        newly_ready.push(dep.clone());
                    }
                }
            }

            // Sort newly ready passes by PassId for stable determinism
            newly_ready.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            for nr in newly_ready {
                queue.push_back(nr);
            }
        }

        if sorted_ids.len() != self.passes.len() {
            return Err("Cyclic dependency detected among registered reflection passes".to_string());
        }

        let result = sorted_ids
            .into_iter()
            .map(|id| Arc::clone(self.passes.get(&id).unwrap()))
            .collect();

        Ok(result)
    }
}
