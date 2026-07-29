//! Unit tests for Milestone B1 two-tier retrieval contracts.

use brain_domain::{EntityId, KnowledgeEvidence, SourceId};
use brain_services::retrieval::contracts::{
    Candidate, CandidateSet, EvidenceItem, EvidenceSet, QueryContext, RankPolicy, Retriever,
    ScoreVector, Scorer,
};
use std::collections::HashMap;

struct MockRetriever;

impl Retriever for MockRetriever {
    fn name(&self) -> &'static str {
        "MockRetriever"
    }

    fn retrieve(&self, query: &QueryContext) -> CandidateSet {
        let mut set = CandidateSet::new();
        set.add(Candidate {
            entity_id: EntityId::new(),
            preferred_name: query.query_string.clone(),
            initial_score: 1.0,
            retriever_source: self.name(),
        });
        set
    }
}

struct MockScorer;

impl Scorer for MockScorer {
    fn name(&self) -> &'static str {
        "MockScorer"
    }

    fn score(&self, candidate: &Candidate, _query: &QueryContext) -> ScoreVector {
        let mut features = HashMap::new();
        features.insert("mock_feature".to_string(), candidate.initial_score);
        ScoreVector { features }
    }
}

struct MockRankPolicy;

impl RankPolicy for MockRankPolicy {
    fn rank(&self, candidates: CandidateSet, scores: Vec<ScoreVector>) -> EvidenceSet {
        let mut items = Vec::new();
        for (i, cand) in candidates.candidates.into_iter().enumerate() {
            let score = scores
                .get(i)
                .and_then(|s| s.features.get("mock_feature"))
                .copied()
                .unwrap_or(0.0);
            items.push(EvidenceItem {
                entity_id: cand.entity_id,
                preferred_name: cand.preferred_name,
                final_score: score,
                evidence: KnowledgeEvidence::default(),
                sources: vec![SourceId("mock_source".to_string())],
            });
        }
        EvidenceSet { items }
    }
}

#[test]
fn test_milestone_b1_contracts_pipeline_flow() {
    let query = QueryContext {
        query_string: "rust relational memory".to_string(),
        limit: 10,
        target_entities: None,
    };

    let retriever = MockRetriever;
    let scorer = MockScorer;
    let ranker = MockRankPolicy;

    let candidate_set = retriever.retrieve(&query);
    assert_eq!(candidate_set.candidates.len(), 1);
    assert_eq!(
        candidate_set.candidates[0].preferred_name,
        "rust relational memory"
    );

    let score_vectors: Vec<_> = candidate_set
        .candidates
        .iter()
        .map(|c| scorer.score(c, &query))
        .collect();
    assert_eq!(score_vectors.len(), 1);
    assert_eq!(score_vectors[0].features.get("mock_feature"), Some(&1.0));

    let evidence_set = ranker.rank(candidate_set, score_vectors);
    assert_eq!(evidence_set.items.len(), 1);
    assert_eq!(evidence_set.items[0].final_score, 1.0);
    assert_eq!(evidence_set.items[0].sources[0].to_string(), "mock_source");
}
