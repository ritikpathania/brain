use serde::{Deserialize, Serialize};
use crate::bkf::errors::BkfError;

/// Operation mode of the Knowledge Processing Pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KppMode {
    /// KPP is completely disabled. Only legacy pipeline runs.
    Disabled,
    /// Legacy writes, KPP compiles in parallel to generate diffs but never writes.
    Shadow,
    /// KPP is active and writes to storage. Legacy pipeline is disabled.
    Active,
}

/// Severity levels for KPP diagnostic logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum KppSeverity {
    /// Informational trace.
    Info,
    /// Non-fatal compile/optimize warning.
    Warning,
    /// Critical compiler pipeline error.
    Error,
}

/// Structured diagnostic log generated during a KPP pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct KppDiagnostic {
    /// Stable machine-readable diagnostic error/warning code.
    pub code: String,
    /// Telemetry severity grade.
    pub severity: KppSeverity,
    /// Human-readable diagnostic explanation.
    pub message: String,
    /// Name of the compiler/optimizer pass that emitted this diagnostic.
    pub origin_pass: Option<String>,
}

/// Output wrapper containing compiled/optimized results and diagnostics metadata.
pub struct PassResult<T> {
    /// Output produced by the pass.
    pub output: T,
    /// Diagnostics logged during the execution of the pass.
    pub diagnostics: Vec<KppDiagnostic>,
}

/// Trait defining a single pure execution pass within the Knowledge Compiler or Optimizer.
pub trait CompilerPass<T>: Send + Sync {
    /// Return the unique static identifier of the pass.
    fn id(&self) -> &'static str;

    /// Runs the pure functional pass on the given input type, returning the output and diagnostics.
    fn run(&self, input: T) -> Result<PassResult<T>, BkfError>;
}

/// Trait defining the parser interface from ObservationIR to KnowledgeIR.
pub trait ObservationParser: Send + Sync {
    /// Parses an `ObservationIR` source into structured pre-optimization `KnowledgeIR`.
    fn parse(&self, observation: &crate::bkf::observation_ir::ObservationIR) -> crate::bkf::ir::KnowledgeIR;
}

/// Default observation parser converting raw ObservationIR sources to KnowledgeIR.
pub struct DefaultObservationParser;

impl ObservationParser for DefaultObservationParser {
    fn parse(&self, observation: &crate::bkf::observation_ir::ObservationIR) -> crate::bkf::ir::KnowledgeIR {
        use crate::bkf::observation_ir::ObservationSource;
        use crate::bkf::ir::{IRNode, IREdge};
        use crate::bkf::lifecycle::{KnowledgeLifecycle, KnowledgeValidity, KnowledgeVersionState};

        let text = match &observation.source {
            ObservationSource::Conversation { prompt, response, .. } => {
                let mut content = prompt.clone();
                if let Some(resp) = response {
                    content.push('\n');
                    content.push_str(resp);
                }
                content
            }
            ObservationSource::File { content, .. } => content.clone(),
            _ => String::new(),
        };

        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // Extract nodes: look for lines matching "entity: [Name] [[Type]]"
        // Extract edges: look for lines matching "relation: [Source] -> [Target] [[Relation]]"
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("entity:") {
                let parts: Vec<&str> = line["entity:".len()..].split('[').collect();
                if !parts.is_empty() {
                    let name = parts[0].trim().to_string();
                    let entity_type = if parts.len() > 1 {
                        parts[1].trim_end_matches(']').trim().to_string()
                    } else {
                        "Concept".to_string()
                    };
                    if !name.is_empty() {
                        let id = format!("node-{}", name.to_lowercase().replace(' ', "-"));
                        nodes.push(IRNode {
                            id,
                            label: name,
                            entity_type,
                            attributes: serde_json::Map::new(),
                            lifecycle: KnowledgeLifecycle::Observed,
                            validity: KnowledgeValidity::Unverified,
                            version_state: KnowledgeVersionState::Current,
                        });
                    }
                }
            } else if line.starts_with("relation:") {
                let content = line["relation:".len()..].trim();
                let parts: Vec<&str> = content.split("->").collect();
                if parts.len() == 2 {
                    let source = parts[0].trim();
                    let right_part = parts[1].trim();
                    let right_parts: Vec<&str> = right_part.split('[').collect();
                    if !right_parts.is_empty() {
                        let target = right_parts[0].trim();
                        let relation = if right_parts.len() > 1 {
                            right_parts[1].trim_end_matches(']').trim().to_string()
                        } else {
                            "related_to".to_string()
                        };
                        if !source.is_empty() && !target.is_empty() {
                            let source_id = format!("node-{}", source.to_lowercase().replace(' ', "-"));
                            let target_id = format!("node-{}", target.to_lowercase().replace(' ', "-"));
                            let edge_id = format!("{}-{}-{}", source_id, target_id, relation.to_lowercase());
                            edges.push(IREdge {
                                id: edge_id,
                                source: source_id,
                                target: target_id,
                                relation,
                                weight: 1.0,
                                lifecycle: KnowledgeLifecycle::Observed,
                                validity: KnowledgeValidity::Unverified,
                                version_state: KnowledgeVersionState::Current,
                            });
                        }
                    }
                }
            }
        }

        crate::bkf::ir::KnowledgeIR { nodes, edges }
    }
}

/// Pass executing node-edge referential validation checks.
pub struct KppValidationPass;

impl CompilerPass<crate::bkf::ir::KnowledgeIR> for KppValidationPass {
    fn id(&self) -> &'static str {
        "kpp_validation_pass"
    }

    fn run(&self, input: crate::bkf::ir::KnowledgeIR) -> Result<PassResult<crate::bkf::ir::KnowledgeIR>, BkfError> {
        let mut diagnostics = Vec::new();
        let node_ids: std::collections::HashSet<&String> = input.nodes.iter().map(|n| &n.id).collect();

        for edge in &input.edges {
            if !node_ids.contains(&edge.source) {
                diagnostics.push(KppDiagnostic {
                    code: "VAL-001".to_string(),
                    severity: KppSeverity::Warning,
                    message: format!("Relationship edge '{}' references missing source node '{}'", edge.id, edge.source),
                    origin_pass: Some(self.id().to_string()),
                });
            }
            if !node_ids.contains(&edge.target) {
                diagnostics.push(KppDiagnostic {
                    code: "VAL-002".to_string(),
                    severity: KppSeverity::Warning,
                    message: format!("Relationship edge '{}' references missing target node '{}'", edge.id, edge.target),
                    origin_pass: Some(self.id().to_string()),
                });
            }
        }

        Ok(PassResult {
            output: input,
            diagnostics,
        })
    }
}

/// Pass performing logical transitive dependency inference.
pub struct KppInferencePass;

impl CompilerPass<crate::bkf::ir::KnowledgeIR> for KppInferencePass {
    fn id(&self) -> &'static str {
        "kpp_inference_pass"
    }

    fn run(&self, mut input: crate::bkf::ir::KnowledgeIR) -> Result<PassResult<crate::bkf::ir::KnowledgeIR>, BkfError> {
        use crate::bkf::ir::IREdge;
        use crate::bkf::lifecycle::{KnowledgeLifecycle, KnowledgeValidity, KnowledgeVersionState};

        let mut diagnostics = Vec::new();
        let mut inferred_edges = Vec::new();

        // Standard transitive dependency inference: A -> B [depends_on] and B -> C [depends_on] => A -> C [depends_on]
        for edge_ab in &input.edges {
            if edge_ab.relation == "depends_on" {
                for edge_bc in &input.edges {
                    if edge_bc.source == edge_ab.target && edge_bc.relation == "depends_on" {
                        // Check if edge A -> C already exists
                        let already_exists = input.edges.iter().any(|e| e.source == edge_ab.source && e.target == edge_bc.target && e.relation == "depends_on");
                        if !already_exists && edge_ab.source != edge_bc.target {
                            let source_id = edge_ab.source.clone();
                            let target_id = edge_bc.target.clone();
                            let edge_id = format!("{}-{}-depends_on-inferred", source_id, target_id);
                            
                            inferred_edges.push(IREdge {
                                id: edge_id.clone(),
                                source: source_id,
                                target: target_id,
                                relation: "depends_on".to_string(),
                                weight: edge_ab.weight * edge_bc.weight * 0.5,
                                lifecycle: KnowledgeLifecycle::Compiled,
                                validity: KnowledgeValidity::Unverified,
                                version_state: KnowledgeVersionState::Current,
                            });

                            diagnostics.push(KppDiagnostic {
                                code: "INF-001".to_string(),
                                severity: KppSeverity::Info,
                                message: format!("Inferred dependency edge '{}' from '{}' and '{}'", edge_id, edge_ab.id, edge_bc.id),
                                origin_pass: Some(self.id().to_string()),
                            });
                        }
                    }
                }
            }
        }

        input.edges.extend(inferred_edges);

        Ok(PassResult {
            output: input,
            diagnostics,
        })
    }
}

/// Orchestrator running observation-to-IR compilation passes.
pub struct KnowledgeCompiler {
    parser: Box<dyn ObservationParser>,
    passes: Vec<Box<dyn CompilerPass<crate::bkf::ir::KnowledgeIR>>>,
}

impl Default for KnowledgeCompiler {
    fn default() -> Self {
        Self::new_default()
    }
}

impl KnowledgeCompiler {
    /// Creates a new `KnowledgeCompiler` with custom parser and passes.
    pub fn new(
        parser: Box<dyn ObservationParser>,
        passes: Vec<Box<dyn CompilerPass<crate::bkf::ir::KnowledgeIR>>>,
    ) -> Self {
        Self { parser, passes }
    }

    /// Creates a default compiler configuration.
    pub fn new_default() -> Self {
        Self::new(
            Box::new(DefaultObservationParser),
            vec![
                Box::new(KppValidationPass),
                Box::new(KppInferencePass),
            ],
        )
    }

    /// Compiles an `ObservationIR` through the parsed representation and registered passes.
    pub fn compile(&self, observation: &crate::bkf::observation_ir::ObservationIR) -> Result<PassResult<crate::bkf::ir::KnowledgeIR>, BkfError> {
        let mut current_ir = self.parser.parse(observation);
        let mut all_diagnostics = Vec::new();

        for pass in &self.passes {
            let res = pass.run(current_ir)?;
            current_ir = res.output;
            all_diagnostics.extend(res.diagnostics);
        }

        Ok(PassResult {
            output: current_ir,
            diagnostics: all_diagnostics,
        })
    }
}


