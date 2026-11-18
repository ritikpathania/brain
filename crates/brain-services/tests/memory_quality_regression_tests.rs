//! Memory Quality & Adversarial Regression Evaluation Suite (Phase 5)

use brain_core::context::{
    ContextAssembler, ContextAssemblyRequest, ContextOrchestrator, MemoryRanker, MemoryScope,
    RetrievalCandidate, RetrievalChannel,
};
use brain_domain::{NodeId, SessionId};
use brain_services::context::{
    DefaultContextAssembler, DefaultContextOrchestrator, DefaultMemoryRanker,
};
use std::collections::HashMap;

#[tokio::test]
async fn test_contradictory_fact_resolution() {
    let ranker = DefaultMemoryRanker::new();
    let assembler = DefaultContextAssembler::new();
    let ref_time = 1_700_000_000;

    let node_old_ws = NodeId::new();
    let node_new_ws = NodeId::new();
    let node_sess_override = NodeId::new();

    // Contradictory facts for API endpoint port:
    // 1. Old workspace setting (port 8080, 5 days ago)
    // 2. Newer workspace setting (port 8081, 1 hour ago)
    // 3. Session specific override (port 9000, now)
    let candidates = vec![
        RetrievalCandidate {
            node_id: node_old_ws,
            label: "API Port Configuration (Old)".to_string(),
            content: "API Gateway listens on port 8080".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.90,
            timestamp: ref_time - (86400 * 5),
            scope: MemoryScope::Workspace("backend-ws".to_string()),
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: node_new_ws,
            label: "API Port Configuration (New)".to_string(),
            content: "API Gateway listens on port 8081".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.90,
            timestamp: ref_time - 3600,
            scope: MemoryScope::Workspace("backend-ws".to_string()),
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: node_sess_override,
            label: "API Port Override (Session)".to_string(),
            content: "API Gateway listens on port 9000".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.90,
            timestamp: ref_time,
            scope: MemoryScope::Session(SessionId::new()),
            metadata: HashMap::new(),
        },
    ];

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);

    // Assert priority: Session Override (9000) > New WS Setting (8081) > Old WS Setting (8080)
    assert_eq!(
        ranked[0].node_id, node_sess_override,
        "Session override must win"
    );
    assert_eq!(
        ranked[1].node_id, node_new_ws,
        "Newer workspace fact must beat stale workspace fact"
    );
    assert_eq!(
        ranked[2].node_id, node_old_ws,
        "Stale fact must rank lowest"
    );

    let snapshot = assembler.assemble(&ranked, 2048, 3, "epoch-conflict");
    assert!(
        snapshot.serialized_context.find("port 9000").unwrap()
            < snapshot.serialized_context.find("port 8081").unwrap()
    );
    assert!(
        snapshot.serialized_context.find("port 8081").unwrap()
            < snapshot.serialized_context.find("port 8080").unwrap()
    );
}

#[tokio::test]
async fn test_tombstoned_latest_fact_composition() {
    let orchestrator = DefaultContextOrchestrator::new();
    let ref_time = 1_700_000_000;
    let node_t0_active = NodeId::new();
    let node_t1_tombstoned = NodeId::new();

    // T0: Database host is db-v1 (valid, older)
    orchestrator
        .register_candidate(RetrievalCandidate {
            node_id: node_t0_active,
            label: "Database Host T0".to_string(),
            content: "Database host: db-v1.internal".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.85,
            timestamp: ref_time - 7200,
            scope: MemoryScope::Workspace("infra".to_string()),
            metadata: HashMap::new(),
        })
        .await;

    // T1: Database host is db-v2 (newer, but tombstoned / deleted)
    orchestrator
        .register_candidate(RetrievalCandidate {
            node_id: node_t1_tombstoned,
            label: "Database Host T1 (Deleted)".to_string(),
            content: "Database host: db-v2.internal".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.95, // Higher raw score & newer timestamp
            timestamp: ref_time - 60,
            scope: MemoryScope::Workspace("infra".to_string()),
            metadata: HashMap::new(),
        })
        .await;

    // Mark T1 as deleted
    orchestrator.mark_node_deleted(node_t1_tombstoned).await;

    let req = ContextAssemblyRequest {
        prompt: "What is the database host?".to_string(),
        session_id: SessionId::new(),
        workspace_id: Some("infra".to_string()),
        max_tokens: 2048,
        max_items: 5,
        reference_time: Some(ref_time),
    };

    let snapshot = orchestrator.assemble_context(req).await.unwrap();

    // Invariant: Tombstoned fact T1 must NEVER appear; T0 remains eligible and is retrieved
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].node_id, node_t0_active);
    assert!(snapshot.serialized_context.contains("db-v1.internal"));
    assert!(
        !snapshot.serialized_context.contains("db-v2.internal"),
        "Tombstoned fact leaked into context!"
    );
}

#[tokio::test]
async fn test_adversarial_near_matches_lexical_distraction() {
    let ranker = DefaultMemoryRanker::new();
    let assembler = DefaultContextAssembler::new();
    let ref_time = 1_700_000_000;

    let node_real = NodeId::new();
    let node_distractor = NodeId::new();

    // Query: "Deploy Kubernetes service"
    // Distractor: lexical overlap on "Kubernetes", but irrelevant semantics ("board game")
    // Real: semantic intent match with deployment instructions
    let candidates = vec![
        RetrievalCandidate {
            node_id: node_distractor,
            label: "Kubernetes Game Rulebook".to_string(),
            content: "Play the Kubernetes board game: roll dice to deploy pods on worker nodes."
                .to_string(),
            channel: RetrievalChannel::Lexical,
            raw_score: 0.70, // High keyword overlap
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: node_real,
            label: "Production Deployment Helm".to_string(),
            content: "Production deployment uses Helm charts to provision services on EKS cluster."
                .to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.92, // High semantic relevance
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
    ];

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);

    assert_eq!(
        ranked[0].node_id, node_real,
        "Semantic match must outrank lexical distractor"
    );
    assert!(ranked[0].final_score > ranked[1].final_score);

    let snapshot = assembler.assemble(&ranked, 2048, 2, "epoch-distractor");
    assert!(
        snapshot.serialized_context.find("Helm charts").unwrap()
            < snapshot.serialized_context.find("board game").unwrap()
    );
}

#[tokio::test]
async fn test_cross_corpus_fuzzy_matching_isolation() {
    let orchestrator = DefaultContextOrchestrator::new();
    let node_alpha = NodeId::new();
    let node_beta = NodeId::new();
    let ref_time = 1_700_000_000;

    orchestrator
        .register_candidate(RetrievalCandidate {
            node_id: node_alpha,
            label: "Auth Token Secret (Alpha)".to_string(),
            content: "Alpha secret: sk_live_ALPHA123".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.95,
            timestamp: ref_time,
            scope: MemoryScope::Workspace("workspace-alpha".to_string()),
            metadata: HashMap::new(),
        })
        .await;

    orchestrator
        .register_candidate(RetrievalCandidate {
            node_id: node_beta,
            label: "Auth Token Secret (Beta)".to_string(),
            content: "Beta secret: sk_live_BETA456".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.95,
            timestamp: ref_time,
            scope: MemoryScope::Workspace("workspace-beta".to_string()),
            metadata: HashMap::new(),
        })
        .await;

    // Query scoped strictly to workspace-alpha
    let req = ContextAssemblyRequest {
        prompt: "Show auth secret".to_string(),
        session_id: SessionId::new(),
        workspace_id: Some("workspace-alpha".to_string()),
        max_tokens: 2048,
        max_items: 5,
        reference_time: Some(ref_time),
    };

    let snapshot = orchestrator.assemble_context(req).await.unwrap();

    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].node_id, node_alpha);
    assert!(snapshot.serialized_context.contains("sk_live_ALPHA123"));
    assert!(
        !snapshot.serialized_context.contains("sk_live_BETA456"),
        "Cross-corpus leakage detected!"
    );
}
