# Phase 4 — Projection Performance & Benchmarking Suite Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-domain` (`benches/projection_benchmarks.rs`)

---

## 1. Executive Summary & Goals

The **Projection Performance & Benchmarking Suite** establishes continuous statistical performance regression testing over all four Phase 4 domain read models (`GraphAdjacencyReducer`, `TemporalStateReducer`, `EntityStatisticsReducer`, and `SearchIndexReducer`) using Criterion:
- **Replay Throughput Group (`events/sec`)**: Evaluates scaling over deterministic streams of 1,000, 10,000, 50,000, and 100,000 generator iterations (measuring throughput over actual emitted `events.len()`).
- **Incremental Mutation Latency Group ($O(1)$ mutation timing)**: Measures single-event latency for `FactRecorded`, `FactSuperseded`, and `FactArchived` operations.
- **Lookup Latency Group ($O(1)$ query timing)**: Measures node neighbor traversal, point-in-time temporal state lookups, entity statistics retrieval, and symmetric search posting lookups.
- **Memory Scaling Group (State footprint per event)**: Reports explicit named metrics: active facts, entities, adjacency edges, indexed tokens, posting-list entries, and estimated bytes per active fact across 10k, 50k, and 100k streams.

All performance tests live in `crates/brain-domain/benches/projection_benchmarks.rs` and remain strictly separate from deterministic correctness tests (`cargo test`). Replay throughput intentionally measures reducer construction, event replay, and final materialized state (excluding runtime orchestration, storage checkpoints, and IPC).

---

## 2. Deterministic Event Stream Generator

The synthetic stream generator emits a 100% reproducible sequence of valid `FactEvent`s with realistic lifecycle proportions (80% `FactRecorded`, 15% `FactSuperseded`, 5% `FactArchived`) enforcing valid BKF ordering. `count` specifies generator loop iterations; throughput measures `events.len()`:

```rust
use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
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
```

---

## 3. Criterion Benchmark Groups (`benches/projection_benchmarks.rs`)

### 1. Replay Throughput (`bench_replay_throughput`)
Evaluates scaling over deterministic streams of 1,000, 10,000, 50,000, and 100,000 events. Uses `criterion::black_box` around inputs and outputs to prevent compiler optimization dead-code elimination.

```rust
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
```

### 2. Incremental Mutation Latency (`bench_incremental_mutation_latency`)
Measures per-event latency for `FactRecorded`, `FactSuperseded`, and `FactArchived` mutations.

### 3. Lookup Latency (`bench_lookup_latency`)
Measures lookup time for `out_edges`, `facts_at`, `get` statistics, and `search_entities`/`search_facts`.

### 4. Memory Scaling (`bench_memory_scaling`)
Reports active facts, entities, adjacency edges, indexed tokens, posting-list entries, and estimated bytes per active fact across 10k, 50k, and 100k streams.

---

## 4. Verification & Command Invocation

Run benchmarks using cargo:
```bash
cargo bench -p brain-domain --bench projection_benchmarks
```
HTML performance report generated in `target/criterion/report/index.html`.
