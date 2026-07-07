# Architecture Documentation

Start here to understand the structural design of the project:

* **[overview.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/overview.md)** — Canonical technical reference guide to the daemon.
* **[STABILITY.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/STABILITY.md)** — Stability contracts for frozen, extensible, and experimental code.
* **[GRAPH_SPEC.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/GRAPH_SPEC.md)** — Specification governing the Knowledge Graph schema and constraints.
* **[relations.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/relations.md)** — Canoncial design specification for relationship semantics and the declarative taxonomy.


## Architectural Decision Records (ADRs)

The core architectural decisions are recorded chronologically in the **[adr/](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr)** directory. They are categorized below by stability and focus:

### Foundational Invariants (Expected Stability: Long-term)
* **[ADR-004: In-Memory Event Bus](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-004.md)** (Accepted) — Establish single-process asynchronous pub-sub for decoupled subsystem events.
* **[ADR-009: BKF Representation](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-009.md)** (Accepted) — Canonical JSON-LD serialization for structured context interchange.
* **[ADR-010: Domain Boundaries](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-010-domain-boundaries.md)** (Accepted) — Isolates `brain-domain` rules from `brain-services` infrastructure.
* **[ADR-011: Immutable Snapshots](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-011-immutable-snapshots.md)** (Accepted) — Configs and weights snapshotting for thread-safety and auditability.
* **[ADR-012: Value Objects](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-012-value-objects.md)** (Accepted) — Wrap floats/strings in validated types to exclude illegal state representation.
* **[ADR-013: Behavioral Invariants](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-013-behavioral-invariants.md)** (Accepted) — Verify retrieval/routing invariants mathematically instead of end-to-end data matches.
* **[ADR-014: Deterministic Execution](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-014-deterministic-execution.md)** (Accepted) — Injecting clocks and stable hash algorithms (`FNV-1a`) to ensure reproducible runs.
* **[ADR-016: Pure Transformations](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-016-pure-transformation-pipelines.md)** (Accepted) — Restructuring execution loops to adhere to stateless `Input -> Transform -> Output` flows.

### Platform & Extensibility (Expected Stability: Medium to High)
* **[ADR-000: Plugin-Based Architecture](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-000.md)** (Accepted) — Define core providers as traits with offloaded blocking executor calls.
* **[ADR-002: GIL Redesign](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-002.md)** (Accepted) — GIL-releasing PyO3 boundaries for low-overhead Python execution.
* **[ADR-006: Retrieval Extension Philosophy](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-006.md)** (Accepted) — Two-path modularity rules freezing pipeline orchestration.
* **[ADR-015: Strategy Interfaces](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-015-strategy-interfaces.md)** (Accepted) — Decouples ranking models and routers behind trait interfaces.
* **[ADR-017: Model Compilation](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-017-model-compilation.md)** (Accepted) — Decouples serializable models from optimized compiled evaluation trees.

### Operational Lifecycle (Expected Stability: Evolutionary)
* **[ADR-007: Streaming API Stability](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-007.md)** (Accepted) — Stability contract for client typewriter network event loops.
* **[ADR-018: Reproducible ML Lifecycle](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-018-reproducible-ml-lifecycle.md)** (Proposed) — Defines promotional checkpoints from feedback and evaluation to canary routing.
* **[ADR-019: Observability First](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-019-observability-first.md)** (Proposed) — Codifying diagnostic reporting, telemetry tracking, and evaluations as core system capabilities.

### Historical Evolutionary Decisions
* **[ADR-001: Native Rust TUI UI](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-001.md)** (Accepted) — Replaced react-ink/bun processes with in-process Thread-isolated Ratatui components.
* **[ADR-003: Deletion of DuckDB OLAP](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-003.md)** (Accepted) — Removed DuckDB synchronizations to reduce deployment latency.
* **[ADR-005: Event Bus Refinements](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-005.md)** (Accepted) — Cleaned event schemas for client network synchronization.
* **[ADR-008: Python Plugin Isolation](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-008.md)** (Accepted) — Standardizes virtual environment setups via `uv`.

## Specifications
* **[rfc/](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc)** — Design RFCs and proposals.
