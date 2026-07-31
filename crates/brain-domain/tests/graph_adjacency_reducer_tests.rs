use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::identifiers::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::*;
use uuid::Uuid;

#[test]
fn test_graph_adjacency_reducer_event_application() {
    let mut reducer =
        GraphAdjacencyReducer::new(ProjectionId::new("graph_adj"), ProjectionVersion(1));
    let fact_id = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());
    let source_entity = KnowledgeEntityId(Uuid::new_v4());
    let target_entity = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact = FactVersion {
        id: fact_id,
        assertion_id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual {
                user_id: "test".to_string(),
            },
            derived_from: vec![],
        },
    };

    let assertion = SemanticAssertion {
        id: assertion_id,
        kind: AssertionKind::Relationship,
        subject: source_entity,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Entity(target_entity),
    };

    let record_event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };
    reducer.apply_event(&record_event).unwrap();

    let node = GraphNodeId(EntityId(source_entity.0));
    assert_eq!(reducer.state().neighbors_out(&node).len(), 1);

    let archive_event = FactEvent::FactArchived {
        fact_id,
        archived_at: Timestamp::now(),
    };
    reducer.apply_event(&archive_event).unwrap();
    assert!(reducer.state().neighbors_out(&node).is_empty());
}
