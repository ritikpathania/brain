use brain_domain::*;

fn create_test_graph() -> (KnowledgeGraph, NodeId, NodeId, NodeId) {
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    graph.add_node(Node::new(node_a, "Rust Lang".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_b, "Cargo Build".to_string(), NodeType::Concept));
    graph.add_node(Node::new(node_c, "Compiler Optimization".to_string(), NodeType::Concept));

    graph.add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 1.0)).unwrap();
    graph.add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 0.9)).unwrap();

    (graph, node_a, node_b, node_c)
}

#[test]
fn test_retrieval_planning_and_optimization() {
    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };
    let compiler = QueryCompiler::new_default();
    let normalized = compiler.compile_legacy(&request).canonical_query;
    
    let planner = RetrievalPlanner;
    let logical_plan = planner.plan(&normalized);

    assert_eq!(logical_plan.steps.len(), 3);

    let optimizer = PlanOptimizer;
    let physical_plan = optimizer.optimize(logical_plan, &CostHeuristics::default());
    assert_eq!(physical_plan.physical_steps.len(), 3);
    assert!(physical_plan.cost.total_cost() > 0.0, "Cost estimation must be calculated");

    // Empty query optimization
    let empty_request = RetrievalRequest {
        query: "   ".to_string(),
        min_confidence: 0.5,
    };
    let logical_empty = planner.plan(&compiler.compile_legacy(&empty_request).canonical_query);
    let physical_empty = optimizer.optimize(logical_empty, &CostHeuristics::default());
    // Should skip Vector and Keyword searches, only keeping neighbor expansion
    assert_eq!(physical_empty.physical_steps.len(), 1);
    assert_eq!(physical_empty.cost.vector_cost, 0.0);
    assert_eq!(physical_empty.cost.keyword_cost, 0.0);
}

#[test]
fn test_sources_and_fusion() {
    let (graph, node_a, node_b, _node_c) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(SnapshotId::new(1), &graph, &registry, &analytics, None);

    // Test VectorSource
    let vec_src = VectorSource::new("Rust".to_string());
    let vec_candidates = vec_src.retrieve(&context);
    assert_eq!(vec_candidates.len(), 1);
    assert_eq!(vec_candidates[0].node_id, node_a);

    // Test GraphExpansionSource with MaxDepth(1) stopping criterion
    let policy = ExpansionPolicy {
        criteria: vec![StoppingCriterion::MaxDepth(1)],
        relation_filter: None,
    };
    let expand_src = GraphExpansionSource::new(seeds_for_test(node_a), policy);
    let expand_candidates = expand_src.retrieve(&context);
    // MaxDepth 1 should expand to node_b but not node_c
    assert_eq!(expand_candidates.len(), 1);
    assert_eq!(expand_candidates[0].node_id, node_b);

    // Test RRF Candidate Fusion
    let keyword_src = KeywordSource::new("Cargo".to_string());
    let kw_candidates = keyword_src.retrieve(&context);
    assert_eq!(kw_candidates.len(), 1);
    assert_eq!(kw_candidates[0].node_id, node_b);

    let fusion = ReciprocalRankFusion::default();
    let fused = fusion.fuse(&[vec_candidates, kw_candidates]);
    assert_eq!(fused.len(), 2);
}

#[test]
fn test_retrieval_invariants_and_explanations() {
    let (graph, node_a, node_b, _node_c) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };
    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let optimizer = PlanOptimizer;
    let plan = optimizer.optimize(planner.plan(&compiler.compile_legacy(&request).canonical_query), &CostHeuristics::default());

    // Clone graph to verify Monotonicity
    let graph_before = graph.clone();
    let result1 = executor.execute(plan.clone(), &NeverCancelled);

    // 1. Monotonicity check
    // Since graph doesn't implement Eq directly but implements clone, we compare nodes and edges.
    assert_eq!(graph.nodes.len(), graph_before.nodes.len(), "Retrieval execution must never mutate the underlying graph");
    assert_eq!(graph.edges.len(), graph_before.edges.len(), "Retrieval execution must never mutate the underlying graph");

    // 2. Determinism check
    let result2 = executor.execute(plan, &NeverCancelled);
    assert_eq!(result1.candidates, result2.candidates, "Retrieval candidate rankings must be deterministic");
    assert_eq!(result1.explanations, result2.explanations, "Retrieval explanations must be deterministic");

    // 3. Observed Cost & Decisions verification
    assert!(result1.report.runtime.candidates_produced > 0);
    assert!(!result1.report.planning.planner_decisions.is_empty());
    assert!(!result1.report.planning.optimizer_decisions.is_empty());

    // 4. Explanation evidence verification
    let explanation_a = result1.explanations.get(&node_a).expect("Explanation for Node A missing");
    let has_semantic = explanation_a.evidence_list.iter().any(|e| matches!(e, Evidence::SemanticMatch { .. }));
    assert!(has_semantic, "Node A should have semantic match evidence");

    let explanation_b = result1.explanations.get(&node_b).expect("Explanation for Node B missing");
    let has_traversal = explanation_b.evidence_list.iter().any(|e| matches!(e, Evidence::GraphTraversal { .. }));
    assert!(has_traversal, "Node B should have graph traversal evidence");
}

#[test]
fn test_declarative_query_dsl() {
    let (graph, _node_a, _node_b, _node_c) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let query_request = QueryRequest {
        semantic_query: "Rust".to_string(),
        min_confidence: 0.8,
        entity_types: None,
        relations: Some(vec![RelationKind::Uses]),
        max_visited: Some(10),
        max_depth: Some(3),
    };

    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let logical_plan = planner.plan(&compiler.compile(&query_request).canonical_query);
    
    // Check that ExpansionPolicy contains the custom criteria
    if let LogicalStep::ExpandNeighbors { policy, .. } = &logical_plan.steps[2] {
        assert!(policy.criteria.contains(&StoppingCriterion::MaxDepth(3)));
        assert!(policy.criteria.contains(&StoppingCriterion::MaxVisitedNodes(10)));
        assert!(policy.criteria.contains(&StoppingCriterion::MinConfidence(0.8)));
        assert_eq!(policy.relation_filter, Some(vec![RelationKind::Uses]));
    } else {
        panic!("Third step must be ExpandNeighbors");
    }

    let optimizer = PlanOptimizer;
    let physical_plan = optimizer.optimize(logical_plan, &CostHeuristics::default());
    let result = executor.execute(physical_plan, &NeverCancelled);

    assert!(result.report.runtime.ranking_operations > 0);
}

#[test]
fn test_query_compilation_determinism_and_synonyms() {
    let compiler = QueryCompiler::new_default();
    
    let req_postgres_lower = QueryRequest {
        semantic_query: "postgres".to_string(),
        min_confidence: 0.5,
        entity_types: None,
        relations: None,
        max_visited: None,
        max_depth: None,
    };
    
    let req_postgres_mixed = QueryRequest {
        semantic_query: "  Postgres ".to_string(),
        min_confidence: 0.5,
        entity_types: None,
        relations: None,
        max_visited: None,
        max_depth: None,
    };
    
    let req_postgres_upper = QueryRequest {
        semantic_query: "POSTGRES".to_string(),
        min_confidence: 0.5,
        entity_types: None,
        relations: None,
        max_visited: None,
        max_depth: None,
    };

    let res_lower = compiler.compile(&req_postgres_lower);
    let res_mixed = compiler.compile(&req_postgres_mixed);
    let res_upper = compiler.compile(&req_postgres_upper);

    // Assert standard casing and trimming normalization
    assert_eq!(res_lower.canonical_query, res_mixed.canonical_query, "Compilation must produce identical normalized queries");
    assert_eq!(res_lower.canonical_query, res_upper.canonical_query, "Compilation must produce identical normalized queries");
    
    // Assert synonym resolution
    assert_eq!(res_lower.canonical_query.semantic_query, "postgresql", "Common synonym postgres must compile to postgresql");
    assert!(res_lower.metadata.passes_executed.contains(&"semantic_rewriter".to_string()));
    assert!(res_lower.metadata.diagnostics.iter().any(|d| d.code == DiagnosticCode::AliasExpanded && d.severity == Severity::Info && d.code.as_str() == "CMP-100" && d.origin_pass == Some("semantic_rewriter".to_string())));

    // Assert identical logical planning
    let planner = RetrievalPlanner;
    let plan1 = planner.plan(&res_lower.canonical_query);
    let plan2 = planner.plan(&res_upper.canonical_query);

    assert_eq!(plan1.steps.len(), plan2.steps.len());
    for (s1, s2) in plan1.steps.iter().zip(plan2.steps.iter()) {
        match (s1, s2) {
            (LogicalStep::VectorRetrieve { query: q1 }, LogicalStep::VectorRetrieve { query: q2 }) => {
                assert_eq!(q1, q2);
            }
            (LogicalStep::KeywordRetrieve { query: q1 }, LogicalStep::KeywordRetrieve { query: q2 }) => {
                assert_eq!(q1, q2);
            }
            _ => {}
        }
    }
}

#[test]
fn test_constant_folding_depth_zero() {
    let req = QueryRequest {
        semantic_query: "Rust".to_string(),
        min_confidence: 0.5,
        entity_types: None,
        relations: None,
        max_visited: None,
        max_depth: Some(0),
    };
    let compiler = QueryCompiler::new_default();
    let res = compiler.compile(&req);
    assert!(res.canonical_query.disable_expansion);
    assert!(res.metadata.diagnostics.iter().any(|d| d.code == DiagnosticCode::ConstantFolded && d.severity == Severity::Warning && d.code.as_str() == "CMP-200" && d.origin_pass == Some("constant_folder".to_string())));

    let planner = RetrievalPlanner;
    let logical_plan = planner.plan(&res.canonical_query);
    // Neighbors expansion step is folded/pruned, leaving only 2 retrieval steps
    assert_eq!(logical_plan.steps.len(), 2);
}

#[test]
fn test_compiler_invalid_pass_configuration() {
    use crate::retrieval::planner::{LexicalNormalizer, SemanticRewriter, ConstantFolder};
    use crate::retrieval::models::CompilerBuildError;

    // 1. Check ordering error (ConstantFolder before SemanticRewriter)
    let bad_order = QueryCompiler::new(vec![
        Box::new(LexicalNormalizer),
        Box::new(ConstantFolder),
        Box::new(SemanticRewriter),
    ]);
    assert!(matches!(bad_order, Err(CompilerBuildError::InvalidPassOrdering { .. })));

    // 2. Check duplicate pass error
    let duplicate_pass = QueryCompiler::new(vec![
        Box::new(LexicalNormalizer),
        Box::new(LexicalNormalizer),
        Box::new(SemanticRewriter),
    ]);
    assert!(matches!(duplicate_pass, Err(CompilerBuildError::DuplicatePass(..))));

    // 3. Check missing Lexical phase error
    let missing_lexical = QueryCompiler::new(vec![
        Box::new(SemanticRewriter),
    ]);
    assert!(matches!(missing_lexical, Err(CompilerBuildError::MissingRequiredPhase(CompilerPhase::Lexical))));

    // 4. Check missing Semantic phase error
    let missing_semantic = QueryCompiler::new(vec![
        Box::new(LexicalNormalizer),
    ]);
    assert!(matches!(missing_semantic, Err(CompilerBuildError::MissingRequiredPhase(CompilerPhase::Semantic))));
}

#[test]
fn test_event_driven_streaming_retrieval() {
    let (graph, _node_a, _node_b, _node_c) = create_test_graph();
    let registry = RelationRegistry::default_embedded();
    let analytics = GraphAnalyticsContext::new(&graph);
    let context = RetrievalExecutionContext::new(SnapshotId::new(1), &graph, &registry, &analytics, None);

    let executor = RetrievalExecutor::new(
        &context,
        ReciprocalRankFusion::default(),
        NormalizedTieBreakerRanking,
    );

    let request = RetrievalRequest {
        query: "Rust".to_string(),
        min_confidence: 0.5,
    };
    let compiler = QueryCompiler::new_default();
    let planner = RetrievalPlanner;
    let optimizer = PlanOptimizer;
    let plan = optimizer.optimize(planner.plan(&compiler.compile_legacy(&request).canonical_query), &CostHeuristics::default());

    // 1. Batch execution
    let batch_result = executor.execute(plan.clone(), &NeverCancelled);

    // 2. Stream execution
    let mut sink = RecordingSink::new();
    let stream_result = executor.execute_stream(plan, &mut sink, &NeverCancelled);

    // Assert Batch Equivalence
    assert_eq!(batch_result.candidates, stream_result.candidates, "Batch and stream results must be identical");

    // Telemetry and lifecycle validations:
    // Check Single Completion
    let completed_events: Vec<&RetrievalEvent> = sink.events().iter()
        .filter(|e| matches!(e, RetrievalEvent::Completed { .. }))
        .collect();
    assert_eq!(completed_events.len(), 1, "Completed event must be emitted exactly once");

    // Check Completed is at the end of the events list
    assert!(matches!(sink.events().last().unwrap(), RetrievalEvent::Completed { .. }), "Last event must be Completed");

    // Check Event Ordering & Nesting
    let mut active_stages = Vec::new();
    let mut stages_seen = Vec::new();
    let mut seen_completed = false;

    for event in sink.events() {
        if seen_completed {
            panic!("Event emitted after Completed event!");
        }

        match event {
            RetrievalEvent::StageStarted { stage } => {
                // Ensure nesting: no other stage should be active
                assert!(active_stages.is_empty(), "Stage started before previous stage completed: {:?}", stage);
                active_stages.push(*stage);
                stages_seen.push(*stage);
            }
            RetrievalEvent::StageCompleted { stage } => {
                let last_active = active_stages.pop().expect("StageCompleted emitted but no stage active");
                assert_eq!(last_active, *stage, "StageCompleted mismatch");
            }
            RetrievalEvent::Completed { .. } => {
                assert!(active_stages.is_empty(), "Completed event emitted while stages are still active: {:?}", active_stages);
                seen_completed = true;
            }
            RetrievalEvent::CandidateFound(_c) => {
                // Ensure candidate found event is emitted inside active retrieval stages
                assert!(!active_stages.is_empty(), "CandidateFound emitted outside of active stage");
                let current_stage = active_stages.last().unwrap();
                assert!(
                    matches!(current_stage, RetrievalStage::VectorSearch | RetrievalStage::KeywordSearch | RetrievalStage::GraphExpansion),
                    "CandidateFound emitted in non-retrieval stage: {:?}", current_stage
                );
            }
            RetrievalEvent::ExplanationUpdated { .. } => {
                assert!(!active_stages.is_empty(), "ExplanationUpdated emitted outside of active stage");
            }
        }
    }

    // Verify all 5 expected stages were executed in correct monotonic order
    assert_eq!(
        stages_seen,
        vec![
            RetrievalStage::VectorSearch,
            RetrievalStage::KeywordSearch,
            RetrievalStage::GraphExpansion,
            RetrievalStage::Fusion,
            RetrievalStage::Ranking
        ]
    );
}

fn seeds_for_test(seed: NodeId) -> Vec<NodeId> {
    vec![seed]
}
