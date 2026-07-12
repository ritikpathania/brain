use crate::bkf::ir::{KnowledgeIR, CompiledKnowledge};
use crate::bkf::compiler::PassResult;
use crate::bkf::errors::BkfError;

/// Trait defining a semantics-preserving optimization pass.
///
/// **Semantic Preservation Invariant**:
/// All implementations of `OptimizerPass` may optimize structures (e.g. merge duplicates, fold aliases)
/// but must never change the underlying semantic meaning of the knowledge graph.
///
/// **Mechanical Optimizer Invariant**:
/// All implementations must be strictly mechanical. All LLMs, heuristics, and subjective logic
/// must be handled within the Reflection phase, not here.
pub trait OptimizerPass: Send + Sync {
    /// Return the unique static identifier of the optimizer pass.
    fn id(&self) -> &'static str;

    /// Runs the pure optimization pass on `CompiledKnowledge` to output optimized `CompiledKnowledge`.
    fn run(&self, input: CompiledKnowledge) -> Result<PassResult<CompiledKnowledge>, BkfError>;
}

/// Pass performing mechanical entity merging, alias folding, and deduplication.
pub struct EntityCanonicalizerPass;

impl OptimizerPass for EntityCanonicalizerPass {
    fn id(&self) -> &'static str {
        "entity_canonicalizer_pass"
    }

    fn run(&self, input: CompiledKnowledge) -> Result<PassResult<CompiledKnowledge>, BkfError> {
        use std::collections::{HashMap, HashSet};
        use crate::bkf::compiler::{KppDiagnostic, KppSeverity};

        let mut diagnostics = Vec::new();
        let mut canonical_nodes = Vec::new();
        let mut label_to_canonical_id: HashMap<String, String> = HashMap::new();
        let mut id_redirects: HashMap<String, String> = HashMap::new();

        // 1. Group nodes by canonical label (lowercase, trimmed)
        for node in input.nodes {
            let canonical_label = node.label.trim().to_lowercase();
            if let Some(canonical_id) = label_to_canonical_id.get(&canonical_label) {
                id_redirects.insert(node.id.clone(), canonical_id.clone());
                diagnostics.push(KppDiagnostic {
                    code: "OPT-001".to_string(),
                    severity: KppSeverity::Info,
                    message: format!("Canonicalizing duplicate node '{}' to canonical ID '{}'", node.id, canonical_id),
                    origin_pass: Some(self.id().to_string()),
                });
            } else {
                label_to_canonical_id.insert(canonical_label, node.id.clone());
                canonical_nodes.push(node);
            }
        }

        // 2. Redirect edge sources and targets to canonical node IDs
        let mut canonical_edges = Vec::new();
        let mut seen_edges = HashSet::new();

        for mut edge in input.edges {
            if let Some(new_source) = id_redirects.get(&edge.source) {
                edge.source = new_source.clone();
            }
            if let Some(new_target) = id_redirects.get(&edge.target) {
                edge.target = new_target.clone();
            }

            // Regenerate edge ID to match canonical sources/targets
            edge.id = format!("{}-{}-{}", edge.source, edge.target, edge.relation.to_lowercase());

            // Deduplicate redundant edges (e.g. from duplicates merging)
            if seen_edges.insert(edge.id.clone()) {
                canonical_edges.push(edge);
            } else {
                diagnostics.push(KppDiagnostic {
                    code: "OPT-002".to_string(),
                    severity: KppSeverity::Info,
                    message: format!("Deduplicated redundant edge '{}'", edge.id),
                    origin_pass: Some(self.id().to_string()),
                });
            }
        }

        Ok(PassResult {
            output: CompiledKnowledge {
                nodes: canonical_nodes,
                edges: canonical_edges,
            },
            diagnostics,
        })
    }
}

/// Orchestrator applying optimizer passes to CompiledKnowledge graphs.
pub struct KnowledgeOptimizer {
    passes: Vec<Box<dyn OptimizerPass>>,
}

impl Default for KnowledgeOptimizer {
    fn default() -> Self {
        Self::new_default()
    }
}

impl KnowledgeOptimizer {
    /// Creates a new `KnowledgeOptimizer`.
    pub fn new(passes: Vec<Box<dyn OptimizerPass>>) -> Self {
        Self { passes }
    }

    /// Creates a default optimizer configuration.
    pub fn new_default() -> Self {
        Self::new(vec![
            Box::new(EntityCanonicalizerPass),
        ])
    }

    /// Optimizes `KnowledgeIR` into `CompiledKnowledge` by chaining all optimization passes.
    pub fn optimize(&self, ir: KnowledgeIR) -> Result<PassResult<CompiledKnowledge>, BkfError> {
        let mut current_graph = CompiledKnowledge::from_ir(ir);
        let mut all_diagnostics = Vec::new();

        for pass in &self.passes {
            let res = pass.run(current_graph)?;
            current_graph = res.output;
            all_diagnostics.extend(res.diagnostics);
        }

        Ok(PassResult {
            output: current_graph,
            diagnostics: all_diagnostics,
        })
    }
}


