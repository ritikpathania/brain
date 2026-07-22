use brain_core::retrieval::{
    DecayModel, RerankContext, Reranker, RetrievalRequest, TemporalRankingSettings,
};
use brain_domain::{Node, NodeId, NodeType};
use brain_services::retrieval::ranking::reranker::TemporalReranker;
use std::collections::HashSet;
use uuid::Uuid;

// Helper to create a dummy node with specific updated_at time
fn make_node(id: NodeId, updated_at: u64) -> Node {
    Node::new(id, format!("node-{}", id.0), NodeType::Concept).with_updated_at(updated_at)
}

// Helper to construct a base RetrievalRequest
fn make_request(reference_time: u64) -> RetrievalRequest {
    RetrievalRequest {
        session_id: brain_domain::SessionId::new(),
        query: "test".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
        reference_time: Some(reference_time),
    }
}

#[test]
fn test_disabled_by_default() {
    let reranker = TemporalReranker::new();

    // Node A (newer but lower initial rank), Node B (older but higher initial rank)
    let node_a = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        1000,
    );
    let node_b = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
        500,
    );

    let candidates = vec![node_b.clone(), node_a.clone()];
    let request = make_request(1000);
    let config = TemporalRankingSettings {
        enabled: false,
        model: DecayModel::Exponential,
        half_life_seconds: 100,
        scaling_factor: 1.0,
    };
    let context = RerankContext {
        request: &request,
        config: &config,
        reference_time: 1000,
    };

    let result = reranker.rerank(candidates.clone(), &context).unwrap();
    // Disabled => order is preserved exactly
    assert_eq!(result[0].id, node_b.id);
    assert_eq!(result[1].id, node_a.id);
}

#[test]
fn test_uniform_decay() {
    let reranker = TemporalReranker::new();

    let node_a = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        1000,
    );
    let node_b = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
        500,
    );

    let candidates = vec![node_b.clone(), node_a.clone()];
    let request = make_request(1000);
    let config = TemporalRankingSettings {
        enabled: true,
        model: DecayModel::Uniform,
        half_life_seconds: 100,
        scaling_factor: 1.0,
    };
    let context = RerankContext {
        request: &request,
        config: &config,
        reference_time: 1000,
    };

    let result = reranker.rerank(candidates.clone(), &context).unwrap();
    // Uniform => order is preserved exactly
    assert_eq!(result[0].id, node_b.id);
    assert_eq!(result[1].id, node_a.id);
}

#[test]
fn test_exponential_decay() {
    let reranker = TemporalReranker::new();

    // Node A (newer but rank 2), Node B (older but rank 1)
    // T = 1000
    // Node A: updated_at = 1000 (dt = 0, raw_decay = 1.0)
    // Node B: updated_at = 800 (dt = 200, half_life = 100 => dt is 2 half-lives => raw_decay = 0.25)
    let node_a = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        1000,
    );
    let node_b = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
        800,
    );

    let candidates = vec![node_b.clone(), node_a.clone()];
    let request = make_request(1000);
    let config = TemporalRankingSettings {
        enabled: true,
        model: DecayModel::Exponential,
        half_life_seconds: 100,
        scaling_factor: 1.0,
    };
    let context = RerankContext {
        request: &request,
        config: &config,
        reference_time: 1000,
    };

    let result = reranker.rerank(candidates, &context).unwrap();
    // Node A is now first because Node B decayed heavily
    assert_eq!(result[0].id, node_a.id);
    assert_eq!(result[1].id, node_b.id);
}

#[test]
fn test_logarithmic_decay() {
    let reranker = TemporalReranker::new();

    // Logarithmic decay drops slower than exponential in the long tail.
    let node_a = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        1000,
    );
    let node_b = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
        800,
    );

    let candidates = vec![node_b.clone(), node_a.clone()];
    let request = make_request(1000);
    let config = TemporalRankingSettings {
        enabled: true,
        model: DecayModel::Logarithmic,
        half_life_seconds: 100,
        scaling_factor: 1.0,
    };
    let context = RerankContext {
        request: &request,
        config: &config,
        reference_time: 1000,
    };

    let result = reranker.rerank(candidates, &context).unwrap();
    // Under logarithmic decay:
    // Node A (dt = 0) -> score ~ base_a * 1.0 = (1/62) * 1.0 = 0.0161
    // Node B (dt = 200) -> score ~ base_b * (1 / (1 + ln(201))) = (1/61) * (1 / (1 + 5.3)) = 0.0163 * 0.158 = 0.0025
    // So Node A is still ranked first.
    assert_eq!(result[0].id, node_a.id);
    assert_eq!(result[1].id, node_b.id);
}

#[test]
fn test_linear_decay() {
    let reranker = TemporalReranker::new();

    // Node A (newer but rank 2), Node B (older but rank 1)
    // T = 1000, half_life_seconds = W = 500
    // Node A: updated_at = 1000 (dt = 0, raw_decay = 1.0)
    // Node B: updated_at = 400 (dt = 600 => dt > W => raw_decay = 0.0)
    let node_a = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        1000,
    );
    let node_b = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
        400,
    );

    let candidates = vec![node_b.clone(), node_a.clone()];
    let request = make_request(1000);
    let config = TemporalRankingSettings {
        enabled: true,
        model: DecayModel::Linear,
        half_life_seconds: 500,
        scaling_factor: 1.0,
    };
    let context = RerankContext {
        request: &request,
        config: &config,
        reference_time: 1000,
    };

    let result = reranker.rerank(candidates, &context).unwrap();
    assert_eq!(result[0].id, node_a.id);
    assert_eq!(result[1].id, node_b.id);
}

#[test]
fn test_clock_skew_clamping() {
    let reranker = TemporalReranker::new();

    // Node A has updated_at in the future relative to request.reference_time (T = 1000, node_a.updated_at = 1200)
    // Future update (skew) clamped to dt = 0 => raw_decay = 1.0.
    let node_a = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        1200,
    );
    let node_b = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
        1000,
    );

    let candidates = vec![node_b.clone(), node_a.clone()];
    let request = make_request(1000);
    let config = TemporalRankingSettings {
        enabled: true,
        model: DecayModel::Exponential,
        half_life_seconds: 100,
        scaling_factor: 1.0,
    };
    let context = RerankContext {
        request: &request,
        config: &config,
        reference_time: 1000,
    };

    let result = reranker.rerank(candidates, &context).unwrap();
    // Node A (clamped to dt = 0, raw_decay = 1.0) gets higher final score than Node B (dt = 0, raw_decay = 1.0, but lower rank)
    // Wait! Let's trace scores:
    // Node A (index 1 => rank 2 => base_score = 1/62 = 0.0161)
    // Node B (index 0 => rank 1 => base_score = 1/61 = 0.0163)
    // Since both have raw_decay = 1.0, Node B should remain first. Let's assert that.
    assert_eq!(result[0].id, node_b.id);
    assert_eq!(result[1].id, node_a.id);
}

#[test]
fn test_deterministic_tie_breaking() {
    let reranker = TemporalReranker::new();

    // Mathematically engineered score tie:
    // Node B at index 0 (rank 1): base_score = 1/61.
    // Node A at index 1 (rank 2): base_score = 1/62.
    // Set half_life_seconds = W = 6200 under Linear decay.
    // Node A: updated_at = 1000 (dt = 0) => decay = 1.0 => score = 1/62.
    // Node B: updated_at = 900 (dt = 100) => decay = 1.0 - 100/6200 = 61/62 => score = (1/61) * (61/62) = 1/62.
    // Both scores tie exactly at 1/62!
    // UUID A (ending in 01) < UUID B (ending in 02).
    // Lexicographical tie-breaking should rank Node A first.
    let node_a = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        1000,
    );
    let node_b = make_node(
        NodeId(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
        900,
    );

    let candidates = vec![node_b.clone(), node_a.clone()];
    let request = make_request(1000);
    let config = TemporalRankingSettings {
        enabled: true,
        model: DecayModel::Linear,
        half_life_seconds: 6200,
        scaling_factor: 1.0,
    };
    let context = RerankContext {
        request: &request,
        config: &config,
        reference_time: 1000,
    };

    let result = reranker.rerank(candidates, &context).unwrap();
    // Tied score => Node A (UUID 01) is sorted before Node B (UUID 02)
    assert_eq!(result[0].id, node_a.id);
    assert_eq!(result[1].id, node_b.id);
}
