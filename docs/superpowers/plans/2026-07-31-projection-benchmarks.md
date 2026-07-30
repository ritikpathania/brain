# Projection Performance & Benchmarking Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Phase 4 **Projection Performance & Benchmarking Suite** (`benches/projection_benchmarks.rs`) in `brain-domain` using Criterion, evaluating replay throughput, incremental mutation latency, lookup latency, and memory scaling across all four domain read models.

**Architecture:** Benches live in `crates/brain-domain/benches/projection_benchmarks.rs` with `harness = false` in `Cargo.toml`. `generate_deterministic_event_stream` creates reproducible streams with valid BKF lifecycle transitions (80% recorded, 15% superseded, 5% archived). Criterion groups `replay_throughput`, `incremental_mutation_latency`, `lookup_latency`, and `memory_scaling` benchmark reducers using `criterion::black_box`.

**Tech Stack:** Rust (edition 2021), `criterion = "0.5"`, `uuid`.

## Global Constraints
- `cargo bench -p brain-domain --bench projection_benchmarks` must compile and execute cleanly.
- Correctness test suites (`cargo test`) remain 100% separate from Criterion performance benchmarks.
- Replay throughput measures `events.len()` elements/sec using `Throughput::Elements`.

---

## Status Tracker

| Milestone | Task | Status | Commit |
| :--- | :--- | :--- | :--- |
| **M1** | Task 1: Cargo.toml Configuration & Deterministic Generator | ✅ Completed | `51f64b1` |
| **M2** | Task 2: Replay Throughput & Incremental Mutation Benchmarks | ✅ Completed | `c447e0e` |
| **M3** | Task 3: Lookup Latency & Memory Scaling Benchmarks | ✅ Completed | `04c457a` |
| **M4** | Task 4: Workspace Verification & Compilation Check | ✅ Completed | `04c457a` |

---

### Task 1: Cargo.toml Configuration & Deterministic Generator

**Files:**
- Modify: `crates/brain-domain/Cargo.toml`
- Create: `crates/brain-domain/benches/projection_benchmarks.rs`

**Interfaces:**
- Consumes: `FactEvent`, `KnowledgeEntityId`, `FactVersionId`, `PredicateId`, `Timestamp`, `FactVersion`, `SemanticAssertion`
- Produces: `generate_deterministic_event_stream(count: usize, entity_cardinality: usize) -> Vec<FactEvent>`

- [ ] **Step 1: Declare benchmark target in Cargo.toml**

```toml
[[bench]]
name = "projection_benchmarks"
harness = false
```

- [ ] **Step 2: Create initial generator structure in `benches/projection_benchmarks.rs`**

```rust
// crates/brain-domain/benches/projection_benchmarks.rs
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

fn dummy_bench(_c: &mut Criterion) {}

criterion_group!(benches, dummy_bench);
criterion_main!(benches);
```

- [ ] **Step 3: Run check to verify compilation**

```bash
cargo check --benches -p brain-domain
```
Expected: PASS cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(benches): configure projection_benchmarks target and deterministic event generator"
```

---

### Task 2: Replay Throughput & Incremental Mutation Benchmarks

**Files:**
- Modify: `crates/brain-domain/benches/projection_benchmarks.rs`

- [ ] **Step 1: Implement `bench_replay_throughput` and `bench_incremental_mutation_latency`**

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
        });
    });

    group.finish();
}
```

- [ ] **Step 2: Run check to verify compilation**

```bash
cargo check --benches -p brain-domain
```
Expected: PASS cleanly.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(benches): add replay_throughput and incremental_mutation_latency benchmark groups"
```

---

### Task 3: Lookup Latency & Memory Scaling Benchmarks

**Files:**
- Modify: `crates/brain-domain/benches/projection_benchmarks.rs`

- [ ] **Step 1: Implement `bench_lookup_latency` and `bench_memory_scaling`**

```rust
fn bench_lookup_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup_latency");
    let events = generate_deterministic_event_stream(10_000, 1_000);
    
    let mut adj_reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let mut temp_reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let mut stats_reducer = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
    let mut search_reducer = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

    for ev in &events {
        let _ = adj_reducer.apply_event(ev);
        let _ = temp_reducer.apply_event(ev);
        let _ = stats_reducer.apply_event(ev);
        let _ = search_reducer.apply_event(ev);
    }

    let target_node = GraphNodeId(EntityId(Uuid::from_u128(1)));
    let target_entity = KnowledgeEntityId(Uuid::from_u128(1));
    let fixed_time = Timestamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000));

    group.bench_function("graph_adj_out_edges", |b| {
        b.iter(|| {
            black_box(adj_reducer.state().out_edges(black_box(&target_node)));
        });
    });

    group.bench_function("temporal_facts_at", |b| {
        b.iter(|| {
            black_box(temp_reducer.state().facts_at(black_box(&target_entity), black_box(fixed_time)));
        });
    });

    group.bench_function("entity_stats_get", |b| {
        b.iter(|| {
            black_box(stats_reducer.state().get(black_box(&target_entity)));
        });
    });

    group.bench_function("search_index_query", |b| {
        b.iter(|| {
            black_box(search_reducer.state().search_entities(black_box("00000000")));
        });
    });

    group.finish();
}

fn bench_memory_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_scaling");
    let scales = [10_000, 50_000, 100_000];

    for &scale in &scales {
        let events = generate_deterministic_event_stream(scale, 1_000);

        group.bench_with_input(BenchmarkId::new("active_elements_count", scale), &events, |b, evs| {
            b.iter(|| {
                let mut adj = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
                let mut temp = TemporalStateReducer::new(ProjectionId::new("temp"), ProjectionVersion(1));
                let mut stats = EntityStatisticsReducer::new(ProjectionId::new("stats"), ProjectionVersion(1));
                let mut search = SearchIndexReducer::new(ProjectionId::new("search"), ProjectionVersion(1));

                for ev in black_box(evs) {
                    let _ = adj.apply_event(ev);
                    let _ = temp.apply_event(ev);
                    let _ = stats.apply_event(ev);
                    let _ = search.apply_event(ev);
                }

                black_box((adj.state().edge_count(), temp.state().record_count(), stats.state().len(), search.state().len()))
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_replay_throughput,
    bench_incremental_mutation_latency,
    bench_lookup_latency,
    bench_memory_scaling
);
criterion_main!(benches);
```

- [ ] **Step 2: Run check to verify compilation**

```bash
cargo check --benches -p brain-domain
```
Expected: PASS cleanly.

- [ ] **Step 3: Commit**

```bash
git add crates/brain-domain/ && git commit -m "feat(benches): add lookup_latency and memory_scaling benchmark groups"
```

---

### Task 4: Workspace Verification & Compilation Check

- Run `cargo check --benches -p brain-domain`.
- Verify clean compilation, 0 warnings.
- Update `walkthrough.md`.
