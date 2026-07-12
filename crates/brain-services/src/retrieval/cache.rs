use brain_domain::retrieval::{
    SnapshotId, QueryRequest, LogicalRetrievalPlan,
    PhysicalRetrievalPlan, CompilationResult, RetrievalResult,
    CompiledQueryCacheKey, LogicalPlanCacheKey, PhysicalPlanCacheKey,
    ResultCacheKey, LayerStats, ExecutionCacheStats, CacheStore, SnapshotCacheStore
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe generator managing monotonic snapshot identities.
#[derive(Debug, Default)]
pub struct SnapshotGenerator {
    counter: AtomicU64,
}

impl SnapshotGenerator {
    /// Creates a new snapshot generator starting at sequence 1.
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(1),
        }
    }

    /// Allocates a new unique SnapshotId.
    pub fn next_snapshot_id(&self) -> SnapshotId {
        SnapshotId::new(self.counter.fetch_add(1, Ordering::Relaxed))
    }
}

/// A concurrent in-memory key-value store powered by Mutex and HashMap.
pub struct InMemoryStore<K, V> {
    map: Mutex<HashMap<K, V>>,
}

impl<K, V> InMemoryStore<K, V> {
    /// Creates a new InMemoryStore.
    pub fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }
}

impl<K, V> Default for InMemoryStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> CacheStore<K, V> for InMemoryStore<K, V>
where
    K: Eq + std::hash::Hash + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn get(&self, key: &K) -> Option<V> {
        let map = self.map.lock().unwrap();
        map.get(key).cloned()
    }

    fn insert(&self, key: K, value: V) {
        let mut map = self.map.lock().unwrap();
        map.insert(key, value);
    }

    fn remove(&self, key: &K) -> Option<V> {
        let mut map = self.map.lock().unwrap();
        map.remove(key)
    }

    fn clear(&self) {
        let mut map = self.map.lock().unwrap();
        map.clear();
    }
}

impl<V> SnapshotCacheStore<CompiledQueryCacheKey, V> for InMemoryStore<CompiledQueryCacheKey, V>
where
    V: Clone + Send + Sync,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        let mut map = self.map.lock().unwrap();
        map.retain(|k, _| k.snapshot_id != snapshot_id);
    }
}

impl<V> SnapshotCacheStore<LogicalPlanCacheKey, V> for InMemoryStore<LogicalPlanCacheKey, V>
where
    V: Clone + Send + Sync,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        let mut map = self.map.lock().unwrap();
        map.retain(|k, _| k.snapshot_id != snapshot_id);
    }
}

impl<V> SnapshotCacheStore<PhysicalPlanCacheKey, V> for InMemoryStore<PhysicalPlanCacheKey, V>
where
    V: Clone + Send + Sync,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        let mut map = self.map.lock().unwrap();
        map.retain(|k, _| k.snapshot_id != snapshot_id);
    }
}

impl<V> SnapshotCacheStore<ResultCacheKey, V> for InMemoryStore<ResultCacheKey, V>
where
    V: Clone + Send + Sync,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        let mut map = self.map.lock().unwrap();
        map.retain(|k, _| k.snapshot_id != snapshot_id);
    }
}

/// Centralized execution cache coordinator managing sub-caches and stats using static generics.
pub struct ExecutionCache<CCompiled, CLogical, CPhysical, CResult>
where
    CCompiled: SnapshotCacheStore<CompiledQueryCacheKey, CompilationResult>,
    CLogical: SnapshotCacheStore<LogicalPlanCacheKey, LogicalRetrievalPlan>,
    CPhysical: SnapshotCacheStore<PhysicalPlanCacheKey, PhysicalRetrievalPlan>,
    CResult: SnapshotCacheStore<ResultCacheKey, (RetrievalResult, PhysicalRetrievalPlan)>,
{
    compiled: CCompiled,
    logical: CLogical,
    physical: CPhysical,
    result: CResult,
    compiled_hits: AtomicU64,
    compiled_misses: AtomicU64,
    logical_hits: AtomicU64,
    logical_misses: AtomicU64,
    physical_hits: AtomicU64,
    physical_misses: AtomicU64,
    result_hits: AtomicU64,
    result_misses: AtomicU64,
}

impl Default for ExecutionCache<
    InMemoryStore<CompiledQueryCacheKey, CompilationResult>,
    InMemoryStore<LogicalPlanCacheKey, LogicalRetrievalPlan>,
    InMemoryStore<PhysicalPlanCacheKey, PhysicalRetrievalPlan>,
    InMemoryStore<ResultCacheKey, (RetrievalResult, PhysicalRetrievalPlan)>,
> {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionCache<
    InMemoryStore<CompiledQueryCacheKey, CompilationResult>,
    InMemoryStore<LogicalPlanCacheKey, LogicalRetrievalPlan>,
    InMemoryStore<PhysicalPlanCacheKey, PhysicalRetrievalPlan>,
    InMemoryStore<ResultCacheKey, (RetrievalResult, PhysicalRetrievalPlan)>,
> {
    /// Creates a new ExecutionCache with default InMemoryStore backends.
    pub fn new() -> Self {
        Self {
            compiled: InMemoryStore::<CompiledQueryCacheKey, CompilationResult>::new(),
            logical: InMemoryStore::<LogicalPlanCacheKey, LogicalRetrievalPlan>::new(),
            physical: InMemoryStore::<PhysicalPlanCacheKey, PhysicalRetrievalPlan>::new(),
            result: InMemoryStore::<ResultCacheKey, (RetrievalResult, PhysicalRetrievalPlan)>::new(),
            compiled_hits: AtomicU64::new(0),
            compiled_misses: AtomicU64::new(0),
            logical_hits: AtomicU64::new(0),
            logical_misses: AtomicU64::new(0),
            physical_hits: AtomicU64::new(0),
            physical_misses: AtomicU64::new(0),
            result_hits: AtomicU64::new(0),
            result_misses: AtomicU64::new(0),
        }
    }
}

impl<CCompiled, CLogical, CPhysical, CResult> ExecutionCache<CCompiled, CLogical, CPhysical, CResult>
where
    CCompiled: SnapshotCacheStore<CompiledQueryCacheKey, CompilationResult>,
    CLogical: SnapshotCacheStore<LogicalPlanCacheKey, LogicalRetrievalPlan>,
    CPhysical: SnapshotCacheStore<PhysicalPlanCacheKey, PhysicalRetrievalPlan>,
    CResult: SnapshotCacheStore<ResultCacheKey, (RetrievalResult, PhysicalRetrievalPlan)>,
{
    /// Creates a custom ExecutionCache with chosen backends.
    pub fn with_stores(
        compiled: CCompiled,
        logical: CLogical,
        physical: CPhysical,
        result: CResult,
    ) -> Self {
        Self {
            compiled,
            logical,
            physical,
            result,
            compiled_hits: AtomicU64::new(0),
            compiled_misses: AtomicU64::new(0),
            logical_hits: AtomicU64::new(0),
            logical_misses: AtomicU64::new(0),
            physical_hits: AtomicU64::new(0),
            physical_misses: AtomicU64::new(0),
            result_hits: AtomicU64::new(0),
            result_misses: AtomicU64::new(0),
        }
    }

    /// Fetches compiled query if present. Increments hit/miss statistics.
    pub fn get_compiled_query(&self, key: &CompiledQueryCacheKey) -> Option<CompilationResult> {
        let found = self.compiled.get(key);
        if found.is_some() {
            self.compiled_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.compiled_misses.fetch_add(1, Ordering::Relaxed);
        }
        found
    }

    /// Inserts compiled query result.
    pub fn insert_compiled_query(&self, key: CompiledQueryCacheKey, val: CompilationResult) {
        self.compiled.insert(key, val);
    }

    /// Fetches logical plan if present.
    pub fn get_logical_plan(&self, key: &LogicalPlanCacheKey) -> Option<LogicalRetrievalPlan> {
        let found = self.logical.get(key);
        if found.is_some() {
            self.logical_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.logical_misses.fetch_add(1, Ordering::Relaxed);
        }
        found
    }

    /// Inserts logical plan.
    pub fn insert_logical_plan(&self, key: LogicalPlanCacheKey, val: LogicalRetrievalPlan) {
        self.logical.insert(key, val);
    }

    /// Fetches physical plan if present.
    pub fn get_physical_plan(&self, key: &PhysicalPlanCacheKey) -> Option<PhysicalRetrievalPlan> {
        let found = self.physical.get(key);
        if found.is_some() {
            self.physical_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.physical_misses.fetch_add(1, Ordering::Relaxed);
        }
        found
    }

    /// Inserts physical plan.
    pub fn insert_physical_plan(&self, key: PhysicalPlanCacheKey, val: PhysicalRetrievalPlan) {
        self.physical.insert(key, val);
    }

    /// Fetches retrieval result if present.
    pub fn get_retrieval_result(&self, key: &ResultCacheKey) -> Option<(RetrievalResult, PhysicalRetrievalPlan)> {
        let found = self.result.get(key);
        if found.is_some() {
            self.result_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.result_misses.fetch_add(1, Ordering::Relaxed);
        }
        found
    }

    /// Inserts retrieval result.
    pub fn insert_retrieval_result(&self, key: ResultCacheKey, val: (RetrievalResult, PhysicalRetrievalPlan)) {
        self.result.insert(key, val);
    }

    /// Invalidates all cached artifacts belonging to the specified SnapshotId.
    pub fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        self.compiled.invalidate_snapshot(snapshot_id);
        self.logical.invalidate_snapshot(snapshot_id);
        self.physical.invalidate_snapshot(snapshot_id);
        self.result.invalidate_snapshot(snapshot_id);
    }

    /// Aggregates and returns hierarchical stats.
    pub fn stats(&self) -> ExecutionCacheStats {
        let compiled = LayerStats {
            hits: self.compiled_hits.load(Ordering::Relaxed),
            misses: self.compiled_misses.load(Ordering::Relaxed),
        };
        let logical = LayerStats {
            hits: self.logical_hits.load(Ordering::Relaxed),
            misses: self.logical_misses.load(Ordering::Relaxed),
        };
        let physical = LayerStats {
            hits: self.physical_hits.load(Ordering::Relaxed),
            misses: self.physical_misses.load(Ordering::Relaxed),
        };
        let result = LayerStats {
            hits: self.result_hits.load(Ordering::Relaxed),
            misses: self.result_misses.load(Ordering::Relaxed),
        };
        let aggregate = LayerStats {
            hits: compiled.hits + logical.hits + physical.hits + result.hits,
            misses: compiled.misses + logical.misses + physical.misses + result.misses,
        };
        ExecutionCacheStats {
            compiled,
            logical,
            physical,
            result,
            aggregate,
        }
    }

    /// Executes the entire retrieval pipeline with layered cache check and short-circuiting.
    pub fn execute_cached<F, R>(
        &self,
        context: &brain_domain::retrieval::RetrievalExecutionContext,
        request: &brain_domain::retrieval::RetrievalRequest,
        heuristics: &brain_domain::retrieval::CostHeuristics,
        compiler: &brain_domain::retrieval::QueryCompiler,
        planner: &brain_domain::retrieval::RetrievalPlanner,
        optimizer: &brain_domain::retrieval::PlanOptimizer,
        executor: &brain_domain::retrieval::RetrievalExecutor<F, R>,
        sink: &mut dyn brain_domain::retrieval::stream::RetrievalSink,
        cancellation: &dyn brain_domain::retrieval::CancellationChecker,
    ) -> RetrievalResult
    where
        F: brain_domain::retrieval::fusion::CandidateFusionStrategy + Send + Sync,
        R: brain_domain::retrieval::ranking::RankingStrategy + Send + Sync,
    {
        use brain_domain::retrieval::stream::{RetrievalEvent, RetrievalStage, CompletionReason};
        use brain_domain::retrieval::{
            PhysicalStep, RetrievedCandidate, Evidence
        };

        if cancellation.is_cancelled() {
            let empty_report = brain_domain::retrieval::RetrievalExecutionReport {
                planning: brain_domain::retrieval::PlanningMetadata {
                    estimated_cost: brain_domain::retrieval::EstimatedCost {
                        vector_cost: 0.0,
                        keyword_cost: 0.0,
                        expansion_cost: 0.0,
                        fusion_cost: 0.0,
                        ranking_cost: 0.0,
                    },
                    planner_decisions: vec![],
                    optimizer_decisions: vec![],
                    heuristics_version: 0,
                },
                runtime: brain_domain::retrieval::RuntimeMetadata {
                    elapsed_microseconds: 0,
                    candidates_produced: 0,
                    candidates_fused: 0,
                    expansions_performed: 0,
                    ranking_operations: 0,
                },
            };
            let empty_result = RetrievalResult {
                candidates: vec![],
                explanations: std::collections::HashMap::new(),
                report: empty_report,
            };
            sink.on_event(RetrievalEvent::Completed {
                reason: CompletionReason::Cancelled,
                result: empty_result.clone(),
            });
            return empty_result;
        }

        let snapshot_id = context.snapshot_id;
        let query_req = QueryRequest {
            semantic_query: request.query.clone(),
            min_confidence: request.min_confidence,
            entity_types: None,
            relations: None,
            max_visited: None,
            max_depth: None,
        };

        let result_key = ResultCacheKey {
            snapshot_id,
            request: query_req.clone(),
        };

        if let Some((result, physical_plan)) = self.get_retrieval_result(&result_key) {
            if cancellation.is_cancelled() {
                let empty_report = brain_domain::retrieval::RetrievalExecutionReport {
                    planning: brain_domain::retrieval::PlanningMetadata {
                        estimated_cost: brain_domain::retrieval::EstimatedCost {
                            vector_cost: 0.0,
                            keyword_cost: 0.0,
                            expansion_cost: 0.0,
                            fusion_cost: 0.0,
                            ranking_cost: 0.0,
                        },
                        planner_decisions: vec![],
                        optimizer_decisions: vec![],
                        heuristics_version: 0,
                    },
                    runtime: brain_domain::retrieval::RuntimeMetadata {
                        elapsed_microseconds: 0,
                        candidates_produced: 0,
                        candidates_fused: 0,
                        expansions_performed: 0,
                        ranking_operations: 0,
                    },
                };
                let empty_result = RetrievalResult {
                    candidates: vec![],
                    explanations: std::collections::HashMap::new(),
                    report: empty_report,
                };
                sink.on_event(RetrievalEvent::Completed {
                    reason: CompletionReason::Cancelled,
                    result: empty_result.clone(),
                });
                return empty_result;
            }
            // Reconstruct and replay events to the sink dynamically
            for step in &physical_plan.physical_steps {
                match step {
                    PhysicalStep::VectorRetrieve { .. } => {
                        let stage = RetrievalStage::VectorSearch;
                        sink.on_event(RetrievalEvent::StageStarted { stage });

                        for scored in &result.candidates {
                            if let Some(explanation) = result.explanations.get(&scored.node_id) {
                                for evidence in &explanation.evidence_list {
                                    if let Evidence::SemanticMatch { similarity } = evidence {
                                        let candidate = RetrievedCandidate {
                                            node_id: scored.node_id,
                                            source_id: "vector",
                                            local_score: *similarity,
                                            explanation_fragments: explanation.evidence_list.clone(),
                                        };
                                        sink.on_event(RetrievalEvent::CandidateFound(candidate));
                                        sink.on_event(RetrievalEvent::ExplanationUpdated {
                                            node_id: scored.node_id,
                                            explanation: explanation.clone(),
                                        });
                                    }
                                }
                            }
                        }

                        sink.on_event(RetrievalEvent::StageCompleted { stage });
                    }
                    PhysicalStep::KeywordRetrieve { .. } => {
                        let stage = RetrievalStage::KeywordSearch;
                        sink.on_event(RetrievalEvent::StageStarted { stage });

                        for scored in &result.candidates {
                            if let Some(explanation) = result.explanations.get(&scored.node_id) {
                                for evidence in &explanation.evidence_list {
                                    if let Evidence::KeywordHit { .. } = evidence {
                                        let candidate = RetrievedCandidate {
                                            node_id: scored.node_id,
                                            source_id: "keyword",
                                            local_score: 0.75,
                                            explanation_fragments: explanation.evidence_list.clone(),
                                        };
                                        sink.on_event(RetrievalEvent::CandidateFound(candidate));
                                        sink.on_event(RetrievalEvent::ExplanationUpdated {
                                            node_id: scored.node_id,
                                            explanation: explanation.clone(),
                                        });
                                    }
                                }
                            }
                        }

                        sink.on_event(RetrievalEvent::StageCompleted { stage });
                    }
                    PhysicalStep::ExpandNeighbors { .. } => {
                        let stage = RetrievalStage::GraphExpansion;
                        sink.on_event(RetrievalEvent::StageStarted { stage });

                        for scored in &result.candidates {
                            if let Some(explanation) = result.explanations.get(&scored.node_id) {
                                for evidence in &explanation.evidence_list {
                                    if let Evidence::GraphTraversal { depth, .. } = evidence {
                                        let score = if *depth > 0 { 0.5 / (*depth as f64) } else { 0.5 };
                                        let candidate = RetrievedCandidate {
                                            node_id: scored.node_id,
                                            source_id: "graph_expansion",
                                            local_score: score,
                                            explanation_fragments: explanation.evidence_list.clone(),
                                        };
                                        sink.on_event(RetrievalEvent::CandidateFound(candidate));
                                        sink.on_event(RetrievalEvent::ExplanationUpdated {
                                            node_id: scored.node_id,
                                            explanation: explanation.clone(),
                                        });
                                    }
                                }
                            }
                        }

                        sink.on_event(RetrievalEvent::StageCompleted { stage });
                    }
                }
            }

            // Fusion Stage Replay
            let stage_fusion = RetrievalStage::Fusion;
            sink.on_event(RetrievalEvent::StageStarted { stage: stage_fusion });
            sink.on_event(RetrievalEvent::StageCompleted { stage: stage_fusion });

            // Ranking Stage Replay
            let stage_ranking = RetrievalStage::Ranking;
            sink.on_event(RetrievalEvent::StageStarted { stage: stage_ranking });
            sink.on_event(RetrievalEvent::StageCompleted { stage: stage_ranking });

            // Completed Event Replay
            sink.on_event(RetrievalEvent::Completed {
                reason: CompletionReason::Finished,
                result: result.clone(),
            });

            result
        } else {
            // Miss in Result Cache: Check intermediate layers

            // 2. Compiled Query Cache
            let compiled_key = CompiledQueryCacheKey {
                snapshot_id,
                request: query_req,
            };
            let compilation_res = if let Some(cached) = self.get_compiled_query(&compiled_key) {
                cached
            } else {
                let compiled = compiler.compile_legacy(request);
                self.insert_compiled_query(compiled_key, compiled.clone());
                compiled
            };

            // 3. Logical Plan Cache
            let logical_key = LogicalPlanCacheKey {
                snapshot_id,
                query: compilation_res.canonical_query.clone(),
            };
            let logical_plan = if let Some(cached) = self.get_logical_plan(&logical_key) {
                cached
            } else {
                let planned = planner.plan(&compilation_res.canonical_query);
                self.insert_logical_plan(logical_key, planned.clone());
                planned
            };

            // 4. Physical Plan Cache
            let physical_key = PhysicalPlanCacheKey {
                snapshot_id,
                plan: logical_plan,
            };
            let physical_plan = if let Some(cached) = self.get_physical_plan(&physical_key) {
                cached
            } else {
                let optimized = optimizer.optimize(physical_key.plan.clone(), heuristics);
                self.insert_physical_plan(physical_key, optimized.clone());
                optimized
            };

            let executed = executor.execute_stream(physical_plan.clone(), sink, cancellation);
            self.insert_retrieval_result(result_key, (executed.clone(), physical_plan));
            executed
        }
    }
}
