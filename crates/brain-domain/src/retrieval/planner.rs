use crate::retrieval::models::{
    CanonicalQuery, CompilationMetadata, CompilationResult, CompilerPhase, Diagnostic,
    DiagnosticCode, ExpansionPolicy, LogicalRetrievalPlan, LogicalStep, QueryRequest,
    RetrievalRequest, Severity, StoppingCriterion,
};

/// Trait defining the contract for query compiler pipeline passes.
///
/// **Compiler Pass Independence Invariant**:
/// Each CompilerPass may transform only the current canonical query and append diagnostics or
/// metadata. A pass must not mutate compiler-global state or depend on hidden side effects
/// from unrelated passes.
pub trait CompilerPass: Send + Sync {
    /// Return the unique static identifier of the pass.
    fn id(&self) -> &'static str;

    /// Declares the structural pipeline phase of the compiler pass.
    fn phase(&self) -> CompilerPhase;

    /// Apply transformations on the CanonicalQuery and record diagnostics.
    fn apply(&self, query: CanonicalQuery, metadata: &mut CompilationMetadata) -> CanonicalQuery;
}

/// Lexical normalizer cleaning casing and whitespaces.
pub struct LexicalNormalizer;

impl CompilerPass for LexicalNormalizer {
    fn id(&self) -> &'static str {
        "lexical_normalizer"
    }

    fn phase(&self) -> CompilerPhase {
        CompilerPhase::Lexical
    }

    fn apply(
        &self,
        mut query: CanonicalQuery,
        metadata: &mut CompilationMetadata,
    ) -> CanonicalQuery {
        metadata.passes_executed.push(self.id().to_string());

        let trimmed = query.semantic_query.trim().to_lowercase();
        // Replace multiple consecutive whitespaces with a single space
        query.semantic_query = trimmed.split_whitespace().collect::<Vec<&str>>().join(" ");

        metadata.diagnostics.push(Diagnostic {
            code: DiagnosticCode::QueryNormalized,
            severity: Severity::Info,
            message: "Lexical casing and whitespace cleaning completed".to_string(),
            origin_pass: Some(self.id().to_string()),
        });

        query
    }
}

/// Semantic rewriter resolving query synonyms.
pub struct SemanticRewriter;

impl CompilerPass for SemanticRewriter {
    fn id(&self) -> &'static str {
        "semantic_rewriter"
    }

    fn phase(&self) -> CompilerPhase {
        CompilerPhase::Semantic
    }

    fn apply(
        &self,
        mut query: CanonicalQuery,
        metadata: &mut CompilationMetadata,
    ) -> CanonicalQuery {
        metadata.passes_executed.push(self.id().to_string());

        if query.semantic_query == "postgres" {
            query.semantic_query = "postgresql".to_string();
            metadata.diagnostics.push(Diagnostic {
                code: DiagnosticCode::AliasExpanded,
                severity: Severity::Info,
                message: "postgres -> postgresql alias expanded".to_string(),
                origin_pass: Some(self.id().to_string()),
            });
        } else if query.semantic_query == "wasm" {
            query.semantic_query = "webassembly".to_string();
            metadata.diagnostics.push(Diagnostic {
                code: DiagnosticCode::AliasExpanded,
                severity: Severity::Info,
                message: "wasm -> webassembly alias expanded".to_string(),
                origin_pass: Some(self.id().to_string()),
            });
        }

        query
    }
}

/// Constant folder optimizing fixed constraint budgets.
pub struct ConstantFolder;

impl CompilerPass for ConstantFolder {
    fn id(&self) -> &'static str {
        "constant_folder"
    }

    fn phase(&self) -> CompilerPhase {
        CompilerPhase::Optimization
    }

    fn apply(
        &self,
        mut query: CanonicalQuery,
        metadata: &mut CompilationMetadata,
    ) -> CanonicalQuery {
        metadata.passes_executed.push(self.id().to_string());

        if query.max_depth == Some(0) || query.max_visited == Some(0) {
            query.disable_expansion = true;
            metadata.diagnostics.push(Diagnostic {
                code: DiagnosticCode::ConstantFolded,
                severity: Severity::Warning,
                message: "graph expansion disabled via zero budget limits".to_string(),
                origin_pass: Some(self.id().to_string()),
            });
        }

        query
    }
}

/// Compiler that normalizes and rewrites declarative QueryRequests to CanonicalQuery representations.
///
/// **Query Compilation Determinism**:
/// Guarantees that equivalent QueryRequests (differing only by casing, whitespace, or synonyms)
/// resolve to identical canonical queries and identical logical plans.
pub struct QueryCompiler {
    passes: Box<[Box<dyn CompilerPass>]>,
}

impl Default for QueryCompiler {
    fn default() -> Self {
        Self::new_default()
    }
}

fn phase_order(phase: CompilerPhase) -> usize {
    match phase {
        CompilerPhase::Lexical => 1,
        CompilerPhase::Semantic => 2,
        CompilerPhase::Optimization => 3,
        CompilerPhase::Validation => 4,
    }
}

impl QueryCompiler {
    /// Constructs a new QueryCompiler with explicit pass list and validates ordering and uniqueness.
    pub fn new(
        passes: Vec<Box<dyn CompilerPass>>,
    ) -> Result<Self, crate::retrieval::models::CompilerBuildError> {
        let mut last_phase_order = 0;
        let mut seen_ids = std::collections::HashSet::new();

        for pass in &passes {
            let pass_id = pass.id();
            if !seen_ids.insert(pass_id) {
                return Err(crate::retrieval::models::CompilerBuildError::DuplicatePass(
                    pass_id.to_string(),
                ));
            }
            let phase = pass.phase();
            let current_order = phase_order(phase);
            if current_order < last_phase_order {
                return Err(
                    crate::retrieval::models::CompilerBuildError::InvalidPassOrdering {
                        pass_id: pass_id.to_string(),
                        phase,
                    },
                );
            }
            last_phase_order = current_order;
        }

        for phase in CompilerPhase::all_phases() {
            if phase.is_required() {
                let has_phase = passes.iter().any(|pass| pass.phase() == *phase);
                if !has_phase {
                    return Err(
                        crate::retrieval::models::CompilerBuildError::MissingRequiredPhase(*phase),
                    );
                }
            }
        }

        Ok(Self {
            passes: passes.into(),
        })
    }

    /// Constructs the default compiler pipeline.
    pub fn new_default() -> Self {
        Self::new(vec![
            Box::new(LexicalNormalizer),
            Box::new(SemanticRewriter),
            Box::new(ConstantFolder),
        ])
        .expect("Default compiler pipeline must be valid")
    }

    /// Compiles a declarative `QueryRequest` into a `CompilationResult`.
    pub fn compile(&self, request: &QueryRequest) -> CompilationResult {
        let mut metadata = CompilationMetadata {
            passes_executed: Vec::new(),
            diagnostics: Vec::new(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        let mut canonical_query = CanonicalQuery {
            semantic_query: request.semantic_query.clone(),
            min_confidence: request.min_confidence,
            entity_types: request.entity_types.clone(),
            relations: request.relations.clone(),
            max_visited: request.max_visited,
            max_depth: request.max_depth,
            disable_expansion: false,
        };

        for pass in self.passes.iter() {
            canonical_query = pass.apply(canonical_query, &mut metadata);
        }

        CompilationResult {
            canonical_query,
            metadata,
        }
    }

    /// Compiles a legacy simple `RetrievalRequest` into a `CompilationResult`.
    pub fn compile_legacy(&self, request: &RetrievalRequest) -> CompilationResult {
        let mut metadata = CompilationMetadata {
            passes_executed: Vec::new(),
            diagnostics: Vec::new(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        let mut canonical_query = CanonicalQuery {
            semantic_query: request.query.clone(),
            min_confidence: request.min_confidence,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
            disable_expansion: false,
        };

        for pass in self.passes.iter() {
            canonical_query = pass.apply(canonical_query, &mut metadata);
        }

        CompilationResult {
            canonical_query,
            metadata,
        }
    }
}

/// Decision planner formulating Logical plans from canonicalized queries.
pub struct RetrievalPlanner;

impl RetrievalPlanner {
    /// Formulates an initial side-effect-free logical plan to satisfy a CanonicalQuery.
    pub fn plan(&self, query: &CanonicalQuery) -> LogicalRetrievalPlan {
        let mut steps = Vec::new();
        steps.push(LogicalStep::VectorRetrieve {
            query: query.semantic_query.clone(),
        });
        steps.push(LogicalStep::KeywordRetrieve {
            query: query.semantic_query.clone(),
        });

        if !query.disable_expansion {
            let mut criteria = Vec::new();
            if let Some(depth) = query.max_depth {
                criteria.push(StoppingCriterion::MaxDepth(depth));
            } else {
                criteria.push(StoppingCriterion::MaxDepth(2));
            }
            if let Some(visited) = query.max_visited {
                criteria.push(StoppingCriterion::MaxVisitedNodes(visited));
            }
            criteria.push(StoppingCriterion::MinConfidence(query.min_confidence));

            let policy = ExpansionPolicy {
                criteria,
                relation_filter: query.relations.clone(),
            };

            steps.push(LogicalStep::ExpandNeighbors {
                source_nodes: Vec::new(),
                policy,
            });
        }

        LogicalRetrievalPlan { steps }
    }
}
