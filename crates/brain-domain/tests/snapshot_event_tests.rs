use brain_domain::bkf::events::*;
use brain_domain::bkf::fact_version::*;
use brain_domain::bkf::snapshot::*;
use brain_domain::bkf::value_objects::*;
use uuid::Uuid;

struct InMemorySnapshot {
    entities: Vec<KnowledgeEntity>,
    assertions: Vec<SemanticAssertion>,
    predicates: Vec<Predicate>,
    active_facts: Vec<FactVersion>,
}

impl KnowledgeSnapshotView for InMemorySnapshot {
    fn entities(&self) -> &[KnowledgeEntity] {
        &self.entities
    }

    fn assertions(&self) -> &[SemanticAssertion] {
        &self.assertions
    }

    fn predicates(&self) -> &[Predicate] {
        &self.predicates
    }

    fn active_facts(&self) -> &[FactVersion] {
        &self.active_facts
    }
}

#[test]
fn test_snapshot_view_trait() {
    let snapshot = InMemorySnapshot {
        entities: vec![],
        assertions: vec![],
        predicates: vec![],
        active_facts: vec![],
    };

    assert_eq!(snapshot.entities().len(), 0);
    assert_eq!(snapshot.assertions().len(), 0);
    assert_eq!(snapshot.predicates().len(), 0);
    assert_eq!(snapshot.active_facts().len(), 0);
}

#[test]
fn test_fact_event_variants() {
    let t1 = Timestamp::now();
    let window = TemporalWindow::new(t1, t1, t1, None).unwrap();
    let confidence = Confidence::new(0.95).unwrap();
    let id1 = FactVersionId(Uuid::new_v4());
    let id2 = FactVersionId(Uuid::new_v4());

    let fact = FactVersion {
        id: id1,
        assertion_id: AssertionId(Uuid::new_v4()),
        lifecycle: FactLifecycle::Candidate,
        confidence,
        temporal: window,
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual {
                user_id: "user1".to_string(),
            },
            derived_from: vec![],
        },
    };

    let recorded = FactEvent::FactRecorded { fact: fact.clone(), assertion: None };
    let _superseded = FactEvent::FactSuperseded {
        old_fact_id: id1,
        new_fact_id: id2,
        superseded_at: t1,
    };
    let _archived = FactEvent::FactArchived {
        fact_id: id1,
        archived_at: t1,
    };

    match recorded {
        FactEvent::FactRecorded { fact: f, .. } => assert_eq!(f.id, id1),
        _ => panic!("Expected FactRecorded"),
    }

}
