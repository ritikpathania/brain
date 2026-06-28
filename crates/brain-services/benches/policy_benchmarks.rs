use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Instant;
use std::sync::Arc;
use brain_domain::{Node, NodeId, NodeType};
use brain_session::StmNode;
use brain_services::conversation::{
    PromotionContext, StmView, SessionMetadata, PromotionPolicy,
    RecencyPolicy, SemanticImportancePolicy, UserPinnedPolicy,
    CompositePolicy, WeightedCompositePolicy, LogicalOp, PropertyImportanceScorer,
};

struct BenchmarkStmView {
    nodes: Vec<StmNode>,
}

impl StmView for BenchmarkStmView {
    fn get_nodes(&self) -> Vec<StmNode> {
        self.nodes.clone()
    }
    fn len(&self) -> usize {
        self.nodes.len()
    }
}

fn generate_mock_stm(size: usize) -> BenchmarkStmView {
    let mut nodes = Vec::with_capacity(size);
    for i in 0..size {
        let mut props = std::collections::HashMap::new();
        props.insert("importance".to_string(), serde_json::json!((i % 10) as f32));
        if i % 25 == 0 {
            props.insert("pinned".to_string(), serde_json::json!(true));
        }
        let mut node = Node::new(NodeId::new(), format!("Label-{}", i), NodeType::Concept);
        node.properties = props;
        nodes.push(StmNode {
            node,
            epoch: brain_session::EpochId(0),
        });
    }
    BenchmarkStmView { nodes }
}

fn bench_policies(c: &mut Criterion) {
    let sizes = [16, 64, 256, 1024];

    let recency = RecencyPolicy::new(Some(500), Some(3600));
    let importance = SemanticImportancePolicy::new(8.0, false, Arc::new(PropertyImportanceScorer));
    let composite = CompositePolicy::new(
        vec![Arc::new(UserPinnedPolicy::new()), Arc::new(RecencyPolicy::new(Some(500), None))],
        LogicalOp::Or,
    );
    let weighted = WeightedCompositePolicy::new(
        vec![
            (Arc::new(UserPinnedPolicy::new()), 4.0),
            (Arc::new(RecencyPolicy::new(Some(500), None)), 3.0),
        ],
        5.0,
    );

    let session_id = brain_domain::SessionId::new();
    let metadata = SessionMetadata::default();
    let now = Instant::now();

    for size in sizes {
        let stm = generate_mock_stm(size);
        let ctx = PromotionContext {
            session_id: &session_id,
            stm: &stm,
            metadata: &metadata,
            now,
        };

        c.bench_function(&format!("recency_policy/size_{}", size), |b| {
            b.iter(|| black_box(recency.should_promote(&ctx)).unwrap())
        });

        c.bench_function(&format!("importance_policy/size_{}", size), |b| {
            b.iter(|| black_box(importance.should_promote(&ctx)).unwrap())
        });

        c.bench_function(&format!("composite_policy/size_{}", size), |b| {
            b.iter(|| black_box(composite.should_promote(&ctx)).unwrap())
        });

        c.bench_function(&format!("weighted_policy/size_{}", size), |b| {
            b.iter(|| black_box(weighted.should_promote(&ctx)).unwrap())
        });
    }
}

criterion_group!(benches, bench_policies);
criterion_main!(benches);
