use brain_application::{BrainApplication, ExecutionContext};
use brain_integrations::dto::v1::SearchQuery;
use brain_services::BrainRuntime;
use std::sync::Arc;

#[tokio::test]
async fn test_search_summary_carries_projection_relevance_score() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("brain_test_app.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = Arc::new(BrainRuntime::new(db_str).expect("Failed to initialize runtime"));

    // Seed a concept node in SQLite storage
    let node_id = brain_domain::NodeId::new();
    let node = brain_domain::Node::new(
        node_id,
        "Alpha Memory Pipeline".to_string(),
        brain_domain::NodeKind::Concept,
    );
    brain_core::repositories::NodeRepository::save(runtime.sqlite_storage().as_ref(), &node)
        .unwrap();

    let app = BrainApplication::new(runtime);
    let context = ExecutionContext::default();

    let query = SearchQuery {
        text: "Alpha".to_string(),
        kinds: Some(vec!["retrieval".to_string()]),
        pagination: None,
    };

    let results = app.search(query, &context).await.unwrap();
    assert!(!results.is_empty(), "Search should find seeded concept");

    let first = &results[0];
    assert_eq!(first.title, "Alpha Memory Pipeline");
    let score_str = first
        .metadata
        .get("score")
        .expect("metadata must contain 'score'");
    let score: i64 = score_str.parse().expect("score must be a valid integer");
    assert!(score > 0, "Score must be positive, got {}", score);
}
