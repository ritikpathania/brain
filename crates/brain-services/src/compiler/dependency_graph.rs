//! Compiler dependency graph for discovering affected subgraphs prior to pass execution.

use crate::compiler::dirty_set::DirtySet;
use crate::compiler::ir::{EntityId, FactId, KnowledgeIR};
use std::collections::{BTreeMap, BTreeSet};

/// Dependency graph tracking entity, fact, and relation connections in Knowledge IR.
#[derive(Debug, Clone, Default)]
pub struct CompilerDependencyGraph {
    /// Maps entity IDs to their subject fact IDs.
    pub entity_to_facts: BTreeMap<EntityId, Vec<FactId>>,
    /// Maps fact IDs to their subject entity ID.
    pub fact_to_entity: BTreeMap<FactId, EntityId>,
    /// Maps entity IDs to relations where they participate as source or target.
    pub entity_to_relations: BTreeMap<EntityId, Vec<(EntityId, EntityId)>>,
    /// Maps lowercased canonical names and aliases to primary EntityId.
    pub alias_to_entity: BTreeMap<String, EntityId>,
}

impl CompilerDependencyGraph {
    /// Builds a dependency graph from a `KnowledgeIR` snapshot.
    pub fn build_from_ir(ir: &KnowledgeIR) -> Self {
        let mut graph = Self::default();

        for (id, entity) in &ir.entities {
            graph
                .alias_to_entity
                .insert(entity.canonical_name.to_lowercase(), id.clone());
            for alias in &entity.aliases {
                graph
                    .alias_to_entity
                    .insert(alias.to_lowercase(), id.clone());
            }
        }

        for (id, fact) in &ir.facts {
            graph
                .entity_to_facts
                .entry(fact.subject_id.clone())
                .or_default()
                .push(id.clone());
            graph
                .fact_to_entity
                .insert(id.clone(), fact.subject_id.clone());
        }

        for rel in &ir.relations {
            let pair = (rel.source_id.clone(), rel.target_id.clone());
            graph
                .entity_to_relations
                .entry(rel.source_id.clone())
                .or_default()
                .push(pair.clone());
            graph
                .entity_to_relations
                .entry(rel.target_id.clone())
                .or_default()
                .push(pair);
        }

        graph
    }

    /// Discovers all affected downstream dependencies and expands the `DirtySet`.
    pub fn expand_dirty_set(&self, input: &DirtySet) -> DirtySet {
        if input.is_full_recompile {
            return input.clone();
        }

        let mut expanded = DirtySet::new(input.graph_version);
        let mut visited_entities = BTreeSet::new();
        let mut queue: Vec<EntityId> = input.dirty_entities.iter().cloned().collect();

        // Include entities from input facts
        for fact_id in &input.dirty_facts {
            expanded.mark_fact(fact_id.clone());
            if let Some(entity_id) = self.fact_to_entity.get(fact_id) {
                queue.push(entity_id.clone());
            }
        }

        // Include relations from input
        for (s, t) in &input.dirty_relations {
            expanded.mark_relation(s.clone(), t.clone());
            queue.push(s.clone());
            queue.push(t.clone());
        }

        // BFS transitive expansion across entity dependency graph
        while let Some(entity_id) = queue.pop() {
            if !visited_entities.insert(entity_id.clone()) {
                continue;
            }

            expanded.mark_entity(entity_id.clone());

            // Add all subject facts for this entity
            if let Some(facts) = self.entity_to_facts.get(&entity_id) {
                for f_id in facts {
                    expanded.mark_fact(f_id.clone());
                }
            }

            // Add all connected relations
            if let Some(rels) = self.entity_to_relations.get(&entity_id) {
                for (s, t) in rels {
                    expanded.mark_relation(s.clone(), t.clone());
                    if !visited_entities.contains(s) {
                        queue.push(s.clone());
                    }
                    if !visited_entities.contains(t) {
                        queue.push(t.clone());
                    }
                }
            }
        }

        expanded
    }
}
