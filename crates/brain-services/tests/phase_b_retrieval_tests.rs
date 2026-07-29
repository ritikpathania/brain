//! Integration tests for Phase B (Milestones B1-B5) two-tier retrieval engine, ranking, and gap analysis.

use brain_domain::{CanonicalEntity, EntityId, KnowledgeEvidence, KnowledgeState};
use brain_services::retrieval::{
    ConfidenceScorer, FtsRetriever, GapAnalyzer, GraphRetriever, LinearRankPolicy, QueryContext,
    RankPolicy, Retriever, Scorer,
};

#[test]
fn test_phase_b_fts_and_graph_retrieval() {
    let id1 = EntityId::new();
    let id2 = EntityId::new();

    let entities = vec![
        CanonicalEntity {
            id: id1,
            preferred_name: "Rust Memory Engine".to_string(),
            aliases: vec!["brain".to_string()],
            merge_history: vec![id2],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Observed,
        },
        CanonicalEntity {
            id: id2,
            preferred_name: "Legacy Memory System".to_string(),
            aliases: vec![],
            merge_history: vec![],
            evidence: KnowledgeEvidence::default(),
            state: KnowledgeState::Archived,
        },
    ];

    let query = QueryContext {
        query_string: "Rust".to_string(),
        limit: 5,
        target_entities: None,
    };

    let fts_retriever = FtsRetriever::new(entities.clone());
    let graph_retriever = GraphRetriever::new(entities);

    let fts_candidates = fts_retriever.retrieve(&query);
    assert_eq!(fts_candidates.candidates.len(), 1);
    assert_eq!(fts_candidates.candidates[0].entity_id, id1);

    let graph_candidates = graph_retriever.retrieve(&query);
    assert_eq!(graph_candidates.candidates.len(), 2); // Includes root entity + merged entity
    assert_eq!(graph_candidates.candidates[0].entity_id, id1);
    assert_eq!(graph_candidates.candidates[1].entity_id, id2);
}

#[test]
fn test_phase_b_linear_ranker_and_scoring() {
    let id1 = EntityId::new();
    let entities = vec![CanonicalEntity {
        id: id1,
        preferred_name: "Knowledge Graph".to_string(),
        aliases: vec![],
        merge_history: vec![],
        evidence: KnowledgeEvidence::default(),
        state: KnowledgeState::Observed,
    }];

    let query = QueryContext {
        query_string: "Knowledge".to_string(),
        limit: 5,
        target_entities: None,
    };

    let fts = FtsRetriever::new(entities);
    let candidates = fts.retrieve(&query);

    let scorer = ConfidenceScorer;
    let scores: Vec<_> = candidates
        .candidates
        .iter()
        .map(|c| scorer.score(c, &query))
        .collect();

    let ranker = LinearRankPolicy::default();
    let evidence_set = ranker.rank(candidates, scores);

    assert_eq!(evidence_set.items.len(), 1);
    assert_eq!(evidence_set.items[0].entity_id, id1);
    assert!(evidence_set.items[0].final_score > 0.0);
}

#[test]
fn test_phase_b_deterministic_gap_analysis() {
    let id1 = EntityId::new();
    let entities = vec![CanonicalEntity {
        id: id1,
        preferred_name: "Compiler Pipeline".to_string(),
        aliases: vec![],
        merge_history: vec![],
        evidence: KnowledgeEvidence::default(),
        state: KnowledgeState::Observed,
    }];

    let query = QueryContext {
        query_string: "Compiler".to_string(),
        limit: 5,
        target_entities: None,
    };

    let fts = FtsRetriever::new(entities);
    let candidates = fts.retrieve(&query);
    let scorer = ConfidenceScorer;
    let scores: Vec<_> = candidates
        .candidates
        .iter()
        .map(|c| scorer.score(c, &query))
        .collect();
    let ranker = LinearRankPolicy::default();
    let evidence_set = ranker.rank(candidates, scores);

    let analyzer = GapAnalyzer::new();
    let gap_report = analyzer.analyze(&query, &evidence_set, &[]);

    assert_eq!(gap_report.query, "Compiler");
    assert_eq!(gap_report.known_facts.len(), 1);
    assert!(gap_report.known_facts[0].contains("Compiler Pipeline"));
    assert!(gap_report.conflicting_evidence.is_empty());

    // Test empty query gap report
    let empty_query = QueryContext {
        query_string: "NonExistentConcept".to_string(),
        limit: 5,
        target_entities: None,
    };
    let empty_report = analyzer.analyze(&empty_query, &Default::default(), &[]);
    assert_eq!(empty_report.known_facts.len(), 0);
    assert_eq!(empty_report.unknown_attributes.len(), 1);
    assert!(!empty_report.suggested_observations.is_empty());
}
