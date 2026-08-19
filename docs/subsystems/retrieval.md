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
  - tui
canonical_specs:
  - docs/product/search-architecture.md
  - docs/architecture/adr/ADR-025-hybrid-retrieval-architecture.md
adrs:
  - ADR-015
  - ADR-025
rfcs:
  - RFC-005
  - RFC-011
---

# Retrieval Engine Subsystem Mini-Handbook

> **Governance Role**: This document is a **Navigation Handbook & Subsystem Summary** (`canonical: false`). Canonical product search requirements live in [`docs/product/search-architecture.md`](../product/search-architecture.md) and algorithm details live in [`ADR-025`](../architecture/adr/ADR-025-hybrid-retrieval-architecture.md).

---

## 1. Purpose
The Retrieval Engine executes multi-channel candidate retrieval (BM25 lexical search and IVF vector similarity search), Reciprocal Rank Fusion (RRF), and temporal decay scoring across the knowledge graph.

## 2. Responsibilities
- Generates candidate entity and fact sets for user queries.
- Combines sparse FTS5 lexical scores and dense BLOB vector embeddings via Reciprocal Rank Fusion (RRF).
- Applies mathematical temporal decay scoring based on edge creation and decay parameters.
- Emits ranked candidate lists to downstream API callers and TUI panels.

## 3. Out of Scope
- Mutating graph topology, facts, or observations (owned by **Compiler**).
- Low-level SQLite FTS5 virtual table indexing or disk WAL writes (owned by **Storage**).
- Terminal viewport rendering (owned by **TUI**).

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

## 8. Implementation Notes
- Uses Reciprocal Rank Fusion constant $k = 60$.
- Uses SIMD-accelerated dot-product calculations for BLOB vector embedding distance computation.

## 9. Canonical References
- [`docs/product/search-architecture.md`](../product/search-architecture.md): Search UX and functional requirements.
- [`ADR-025`](../architecture/adr/ADR-025-hybrid-retrieval-architecture.md): Canonical hybrid retrieval architecture specification.
- [`docs/reference/benchmarking.md`](../reference/benchmarking.md): Retrieval performance harness.

## 10. Related ADRs
- [`ADR-015: Strategy Interfaces`](../architecture/adr/ADR-015-strategy-interfaces.md)
- [`ADR-025: Hybrid Retrieval Architecture`](../architecture/adr/ADR-025-hybrid-retrieval-architecture.md)

## 11. Related RFCs
- [`RFC-005: Hybrid Search`](../architecture/rfc/RFC-005.md)
- [`RFC-011: Temporal Ranking`](../architecture/rfc/RFC-011-temporal-ranking.md)

## 12. Operations
- Query latency target: $< 50\text{ ms}$ for combined RRF retrieval.

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
└── Visualized by: TUI (brain-tui)
```
