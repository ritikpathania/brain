# ADR-026: Graph-Aware Retrieval, Relationship Expansion, and Projections

## Status
Accepted

## Context
Milestone v0.8 focused on layering graph traversal, relationship enrichment, and algorithm-based projections on top of the stable v0.7 retrieval engine. We needed to define how these capabilities integrate with the existing retrieval pipeline while maintaining backward compatibility, performance boundaries, and domain isolation.

Specifically, we resolved the following design decisions:
1. **Graph Traversal Budget**: How to configure and limit BFS graph traversal depth.
2. **Relationship Enrichment**: How to return node connections without leaking domain entities or polluting retrieval candidate counts.
3. **Graph Projections**: How to structure algorithms like shortest paths or community clustering.
4. **Historical Projections**: How to query entire graph state at a target timestamp `T`.

---

## Decisions

### 1. Request-Scoped Graph Traversal Depth
We configured traversal depth on a per-request basis (`RetrievalRequest::graph_depth: Option<usize>`) rather than a global workspace/pipeline policy.
- `None` (default) acts as depth-1 (backward-compatible retrieval).
- `Some(0)` acts as flat retrieval (no expansion).
- `Some(n)` triggers BFS graph-traversal expansion.

**Rationale**: Request-scoped configuration makes latency, quality, and traversal trade-offs completely explicit and visible to client applications. It enables side-by-side A/B benchmarking across different depths without redeploying the daemon.

### 2. Post-Retrieval Relationship Expansion DTOs
We decoupled edge mapping into a post-retrieval pipeline step triggered via `expand_relations: bool`.
- We introduced `RelationshipExpansionDTO` and `EdgeDTO` in `brain-core/src/graph.rs` to serve as the interface representation.
- The `RelationshipExpander` service runs **after** candidate truncation, fetching and grouping first-order connections into incoming and outgoing DTO lists.

**Rationale**: Keeping raw domain entities (`brain_domain::Edge`, `brain_domain::Node`) inside the domain/services layer protects internal invariants and ensures future protocol independence. Executing expansion after truncation avoids the database read overhead of mapping edges for discarded candidates.

### 3. Unified Read Model Projections
We implemented graph-based algorithms (`NeighborhoodProjector`, `PathProjector`, `ClusterProjector`) under the existing `Projector` trait rather than defining new traits or routing mechanisms.

**Rationale**: The `Projector` trait is the canonical read-model boundary. Fitting all graph algorithms into this trait enforces that projections are strictly read-only, side-effect-free, and executed against the authoritative in-memory graph.

### 4. Historical Snapshot Projection
We extended the static `TemporalProjector` with `project_graph(graph, temporal_edges, query)`. This method filters nodes and active/visible edges to reconstruct the exact state of the `KnowledgeGraph` at a target timestamp `T` under visibility constraints (`Current`, `Historical`, `Interval`).

**Rationale**: Bounded-time graph queries are projection concerns. Isolating this logic within `TemporalProjector` prevents temporal filtering rules from leaking into the core retrieval pipeline or the ranker, keeping them highly focused and testable.

---

## Architectural Invariants

The implementation of v0.8 enforces and preserves these fundamental guarantees:
- **Retrieval remains deterministic**: For any given graph state and query, candidate ranking is stable and reproducible.
- **RRF is the sole production fusion strategy**: Reciprocal Rank Fusion is the only strategy used to merge independent lexical and vector candidate streams.
- **Projection never mutates retrieval results**: Projectors are pure, side-effect-free transformations on in-memory graph states.
- **Relationship expansion is optional and post-retrieval**: Enrichment occurs after truncation to minimize overhead.
- **Domain entities never cross service boundaries**: External communication is conducted strictly through serialization-friendly DTOs.
- **RetrievalRequest defaults preserve backward compatibility**: Unpopulated new fields yield behavior identical to the v0.7 engine.
- **Graph traversal is request-scoped**: Depth bounds are defined per-request, preventing global constraints from restricting experimentation.
- **Temporal projection is separate from temporal ranking**: Historical graph queries are isolated within projections, distinct from recency decay scoring logic.

---

## Consequences
- **Backward Compatibility**: Full parity with v0.7 is maintained; retrieval-check gates pass with zero quality/behavioral delta.
- **Performance Boundaries**: Edge fetching scales only with the final page limit size rather than the raw candidate set.
- **Testability**: BFS traversal, shortest path, connected components, and historical snapping are fully isolated and covered by deterministic integration tests.
