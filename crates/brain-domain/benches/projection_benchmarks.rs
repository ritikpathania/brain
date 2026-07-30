//! Performance benchmark suite for Phase 4 domain projections.

use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::identifiers::*;
use brain_domain::projection::entity_statistics::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::search_index::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

/// Generates a 100% deterministic synthetic stream of FactEvents with realistic lifecycle distribution:
/// 80% FactRecorded, 15% FactSuperseded (emits FactRecorded for new_fact before superseding old_fact), 5% FactArchived.
pub fn generate_deterministic_event_stream(count: usize, entity_cardinality: usize) -> Vec<FactEvent> {
    let mut events = Vec::with_capacity(count * 2);
    let fixed_time = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));

    let mut entities = Vec::with_capacity(entity_cardinality);
    for i in 0..entity_cardinality {
        entities.push(KnowledgeEntityId(Uuid::from_u128(i as u128 + 1)));
    }

    let mut active_facts: Vec<(FactVersionId, KnowledgeEntityId, PredicateId)> = Vec::new();

    for i in 0..count {
        let modulo = i % 100;
        if (80..95).contains(&modulo) && !active_facts.is_empty() {
            // 15% FactSuperseded: Emit FactRecorded for new_fact, then FactSuperseded old_fact -> new_fact
            let (old_id, subject, predicate_id) = active_facts.pop().unwrap();
            let new_id = FactVersionId(Uuid::from_u128(i as u128 + 1_000_000));
            let assertion_id = AssertionId(Uuid::from_u128(i as u128 + 5_000_000));
            let target = entities[(i + 2) % entities.len()].clone();

            let new_fact = FactVersion {
                id: new_id.clone(),
                assertion_id,
                lifecycle: FactLifecycle::Verified,
                confidence: Confidence::new(0.95).unwrap(),
                temporal: TemporalWindow::new(fixed_time, fixed_time, fixed_time, None).unwrap(),
                supersedes: Some(old_id.clone()),
                provenance: FactProvenance {
                    source: FactProvenanceSource::Manual { user_id: "bench_runner".to_string() },
                    derived_from: vec![],
                },
            };

            let new_assertion = SemanticAssertion {
                id: assertion_id,
                kind: AssertionKind::Relationship,
                subject: subject.clone(),
                predicate: predicate_id.clone(),
                object: AssertionTarget::Entity(target),
            };

            events.push(FactEvent::FactRecorded {
                fact: new_fact,
                assertion: Some(new_assertion),
            });

            active_facts.push((new_id.clone(), subject, predicate_id));

            events.push(FactEvent::FactSuperseded {
                old_fact_id: old_id,
                new_fact_id: new_id,
                superseded_at: fixed_time,
            });
        } else if modulo >= 95 && !active_facts.is_empty() {
            // 5% FactArchived
            let (fact_id, _, _) = active_facts.pop().unwrap();
            events.push(FactEvent::FactArchived {
                fact_id,
                archived_at: fixed_time,
            });
        } else {
            // 80% FactRecorded
            let subject = entities[i % entities.len()].clone();
            let target = entities[(i + 1) % entities.len()].clone();
            let fact_id = FactVersionId(Uuid::from_u128(i as u128 + 1_000_000));
            let assertion_id = AssertionId(Uuid::from_u128(i as u128 + 5_000_000));
            let predicate_id = PredicateId(Uuid::from_u128((i % 20) as u128 + 100));

            active_facts.push((fact_id.clone(), subject.clone(), predicate_id.clone()));

            let fact = FactVersion {
                id: fact_id,
                assertion_id,
                lifecycle: FactLifecycle::Verified,
                confidence: Confidence::new(0.95).unwrap(),
                temporal: TemporalWindow::new(fixed_time, fixed_time, fixed_time, None).unwrap(),
                supersedes: None,
                provenance: FactProvenance {
                    source: FactProvenanceSource::Manual { user_id: "bench_runner".to_string() },
                    derived_from: vec![],
                },
            };

            let assertion = SemanticAssertion {
                id: assertion_id,
                kind: AssertionKind::Relationship,
                subject,
                predicate: predicate_id,
                object: AssertionTarget::Entity(target),
            };

            events.push(FactEvent::FactRecorded {
                fact,
                assertion: Some(assertion),
            });
        }
    }

    events
}

fn dummy_bench(_c: &mut Criterion) {}

criterion_group!(benches, dummy_bench);
criterion_main!(benches);
