---
status: active
owner: retrieval
canonical: false
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
subsystem: retrieval
owns:
  - crates/brain-services
depends_on:
  - storage
  - domain
used_by:
  - daemon
  - frontend
canonical_specs:
  - docs/architecture/overview.md
  - docs/architecture/adr/ADR-025-hybrid-retrieval-architecture.md
adrs:
  - ADR-025
rfcs:
  - RFC-011
---

# Retrieval Engine Subsystem Mini-Handbook

> **Governance Role**: This document is a **Navigation Handbook & Subsystem Summary** (`canonical: false`). Canonical retrieval architecture details live in [`docs/architecture/overview.md`](../architecture/overview.md) and [`ADR-025`](../architecture/adr/ADR-025-hybrid-retrieval-architecture.md).

---

## 1. Purpose
The Retrieval Engine executes multi-channel candidate retrieval (BM25 lexical search and IVF vector similarity search), Reciprocal Rank Fusion (RRF), and temporal decay scoring across the knowledge graph.

## 2. Responsibilities
- Generates candidate entity and fact sets for user queries.
- Combines sparse FTS5 lexical scores and dense BLOB vector embeddings via Reciprocal Rank Fusion (RRF).
- Applies mathematical temporal decay scoring based on edge creation and decay parameters.
- Emits ranked candidate lists to downstream API callers and frontend adapters.

## 3. Out of Scope
- Mutating graph topology, facts, or observations (owned by **Compiler**).
- Low-level SQLite FTS5 virtual table indexing or disk WAL writes (owned by **Storage**).
- Terminal rendering and presentation state (owned by the first-party frontend).

## 4. Architecture Overview
```text
                     Query Input
                          │
         ┌────────────────┴────────────────┐
         ▼                                 ▼
┌──────────────────┐             ┌──────────────────┐
│ BM25 Lexical FTS5│             │ IVF Vector Search│
└────────┬─────────┘             └────────┬─────────┘
         │ Candidate Set 1                │ Candidate Set 2
         └────────────────┬───────────────┘
                          ▼
             ┌─────────────────────────┐
             │ Reciprocal Rank Fusion  │  (RRF)
             └────────────┬────────────┘
                          ▼
             ┌─────────────────────────┐
             │ Temporal & Decay Rescoring│
             └────────────┬────────────┘
                          ▼
                    Final Ranker
```

## 5. Runtime Flow
1. **Query Dispatch**: `brain query` or IPC client sends query string.
2. **Channel Evaluation**: BM25 FTS5 search and IVF vector search execute concurrently.
3. **Fusion & Rescoring**: RRF combines candidate ranks; temporal decay models re-weight candidates.
4. **Result Delivery**: Top-$K$ candidate list is returned.

## 6. Key Invariants
- **Read-Only Purity**: Retrieval operations never mutate underlying database state.
- **Score Monotonicity**: Candidate scores fall within range $[0.0, 1.0]$.

## 7. Owning Crates
- [`crates/brain-services`](../../crates/brain-services/README.md): Candidate generators, RRF fusion engine, temporal scorers.

## Ranking and model contracts

Use validated domain value objects for weights, scores and tree parameters. Feature normalization, scoring, and offline evaluation use immutable inputs. Replacement of active weights uses a whole snapshot so requests see one version. The serialized tree definition stays separate from the compiled inference layout.

Sticky experiment routing uses 64-bit FNV-1a over a stable key. Missing keys select the active baseline. Regression checks cover normalization bounds, independent feature effects, monotonic ranking metrics and deterministic cohort assignment.

## 8. Implementation Notes
- Uses Reciprocal Rank Fusion constant $k = 60$.
- Uses SIMD-accelerated dot-product calculations for BLOB vector embedding distance computation.

## 9. Canonical References
- [`docs/architecture/overview.md`](../architecture/overview.md): Search and retrieval architecture overview.
- [`ADR-025`](../architecture/adr/ADR-025-hybrid-retrieval-architecture.md): Canonical hybrid retrieval architecture specification.
- [Benchmark guide](../benchmarks/README.md) and [retrieval baselines](../reference/retrieval-baselines/README.md).

## 10. Related ADRs
- [`ADR-025: Hybrid Retrieval Architecture`](../architecture/adr/ADR-025-hybrid-retrieval-architecture.md)

## 11. Related RFCs
- [`RFC-011: Temporal Ranking`](../architecture/rfc/RFC-011-temporal-ranking.md)

## 12. Operations
- Query latency target: under 50 ms for combined RRF retrieval. Record corpus size, hardware and cache state when measuring it.

## 13. Testing
- Criterion benchmarks in `crates/brain-services/benches/` measure scoring throughput.

## 14. Extension Points
- Implement `CandidateScorer` or `RankingStrategy` traits in `brain-services`.

## 15. Subsystem Dependencies
```text
Retrieval Subsystem
├── Depends on: Storage (brain-storage) & Domain (brain-domain)
├── Reads from: Knowledge Graph (brain-storage)
├── Exposed through: IPC Protocol (daemon)
└── Consumed by: protocol clients and first-party frontend adapters
```
