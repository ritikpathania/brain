use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;
use uuid::Uuid;

#[test]
fn test_graph_adjacency_runtime_replay_equivalence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(GraphAdjacencyReducer::new(
        ProjectionId::new("graph_adj"),
        ProjectionVersion(1),
    ));
    let instance = ProjectionInstance::new(reducer);
    runtime.register_projection(instance).unwrap();

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

    let events = [FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    }];
    runtime.catchup_all(events.iter(), Watermark(1)).unwrap();
}

#[test]
fn test_graph_adjacency_mixed_event_sequence_and_degree_invariants() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(GraphAdjacencyReducer::new(
        ProjectionId::new("graph_adj_mixed"),
        ProjectionVersion(1),
    ));
    let instance = ProjectionInstance::new(reducer);
    runtime.register_projection(instance).unwrap();

    let fact_id1 = FactVersionId(Uuid::new_v4());
    let fact_id2 = FactVersionId(Uuid::new_v4());
    let assertion_id1 = AssertionId(Uuid::new_v4());
    let assertion_id2 = AssertionId(Uuid::new_v4());
    let e1 = KnowledgeEntityId(Uuid::new_v4());
    let e2 = KnowledgeEntityId(Uuid::new_v4());
    let e3 = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact1 = FactVersion {
        id: fact_id1,
        assertion_id: assertion_id1,
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
    let assertion1 = SemanticAssertion {
        id: assertion_id1,
        kind: AssertionKind::Relationship,
        subject: e1,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Entity(e2),
    };

    let fact2 = FactVersion {
        id: fact_id2,
        assertion_id: assertion_id2,
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
    let assertion2 = SemanticAssertion {
        id: assertion_id2,
        kind: AssertionKind::Relationship,
        subject: e1,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Entity(e3),
    };

    let events = [
        FactEvent::FactRecorded {
            fact: fact1,
            assertion: Some(assertion1),
        },
        FactEvent::FactRecorded {
            fact: fact2,
            assertion: Some(assertion2),
        },
        FactEvent::FactSuperseded {
            old_fact_id: fact_id1,
            new_fact_id: fact_id2,
            superseded_at: now,
        },
        FactEvent::FactArchived {
            fact_id: fact_id2,
            archived_at: now,
        },
    ];

    runtime.catchup_all(events.iter(), Watermark(4)).unwrap();
}
