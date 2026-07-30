//! Performance benchmark suite for Phase 4 domain projections.

use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
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
            let (fact_id, _, _) = active_facts.pop().unwrap();
            events.push(FactEvent::FactArchived {
                fact_id,
                archived_at: fixed_time,
            });
        } else {
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

fn bench_replay_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("replay_throughput");
    let scales = [1_000, 10_000, 50_000, 100_000];

    for &scale in &scales {
        let events = generate_deterministic_event_stream(scale, 1_000);
        group.throughput(Throughput::Elements(events.len() as u64));

        group.bench_with_input(BenchmarkId::new("graph_adjacency", scale), &events, |b, evs| {
            b.iter(|| {
                let mut reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
                for ev in black_box(evs) {
                    let _ = reducer.apply_event(ev);
                }
                black_box(reducer);
            });
        });

        group.bench_with_input(BenchmarkId::new("temporal_state", scale), &events, |b, evs| {
            b.iter(|| {
                let mut reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
                for ev in black_box(evs) {
                    let _ = reducer.apply_event(ev);
                }
                black_box(reducer);
            });
        });

        group.bench_with_input(BenchmarkId::new("entity_statistics", scale), &events, |b, evs| {
            b.iter(|| {
                let mut reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
                for ev in black_box(evs) {
                    let _ = reducer.apply_event(ev);
                }
                black_box(reducer);
            });
        });

        group.bench_with_input(BenchmarkId::new("search_index", scale), &events, |b, evs| {
            b.iter(|| {
                let mut reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));
                for ev in black_box(evs) {
                    let _ = reducer.apply_event(ev);
                }
                black_box(reducer);
            });
        });
    }

    group.finish();
}

fn bench_incremental_mutation_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_mutation_latency");
    let fixed_time = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    let subject = KnowledgeEntityId(Uuid::from_u128(1));
    let target = KnowledgeEntityId(Uuid::from_u128(2));

    group.bench_function("fact_recorded_graph", |b| {
        let mut reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
        let mut i = 0u128;
        b.iter(|| {
            i += 1;
            let fact_id = FactVersionId(Uuid::from_u128(i));
            let assertion_id = AssertionId(Uuid::from_u128(i));
            let event = FactEvent::FactRecorded {
                fact: FactVersion {
                    id: fact_id,
                    assertion_id,
                    lifecycle: FactLifecycle::Verified,
                    confidence: Confidence::new(0.9).unwrap(),
                    temporal: TemporalWindow::new(fixed_time, fixed_time, fixed_time, None).unwrap(),
                    supersedes: None,
                    provenance: FactProvenance {
                        source: FactProvenanceSource::Manual { user_id: "bench".to_string() },
                        derived_from: vec![],
                    },
                },
                assertion: Some(SemanticAssertion {
                    id: assertion_id,
                    kind: AssertionKind::Relationship,
                    subject: subject.clone(),
                    predicate: PredicateId(Uuid::from_u128(100)),
                    object: AssertionTarget::Entity(target.clone()),
                }),
            };
            let _ = reducer.apply_event(black_box(&event));
            black_box(&reducer);
        });
    });

    group.bench_function("fact_recorded_temporal", |b| {
        let mut reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
        let mut i = 0u128;
        b.iter(|| {
            i += 1;
            let fact_id = FactVersionId(Uuid::from_u128(i));
            let assertion_id = AssertionId(Uuid::from_u128(i));
            let event = FactEvent::FactRecorded {
                fact: FactVersion {
                    id: fact_id,
                    assertion_id,
                    lifecycle: FactLifecycle::Verified,
                    confidence: Confidence::new(0.9).unwrap(),
                    temporal: TemporalWindow::new(fixed_time, fixed_time, fixed_time, None).unwrap(),
                    supersedes: None,
                    provenance: FactProvenance {
                        source: FactProvenanceSource::Manual { user_id: "bench".to_string() },
                        derived_from: vec![],
                    },
                },
                assertion: Some(SemanticAssertion {
                    id: assertion_id,
                    kind: AssertionKind::Relationship,
                    subject: subject.clone(),
                    predicate: PredicateId(Uuid::from_u128(100)),
                    object: AssertionTarget::Entity(target.clone()),
                }),
            };
            let _ = reducer.apply_event(black_box(&event));
            black_box(&reducer);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_replay_throughput,
    bench_incremental_mutation_latency
);
criterion_main!(benches);
