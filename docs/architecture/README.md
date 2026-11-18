# Architecture references

Start with the [root architecture](../../ARCHITECTURE.md), [runtime walkthrough](overview.md), and [request flows](data-flow.md). Follow the [engineering instructions](../../AGENTS.md) for changes to architectural boundaries and the [test matrix](../../TEST_INFRA.md) for executable checks.

- [Graph invariants](GRAPH_SPEC.md) and [relationship taxonomy](relations.md)
- [Compatibility policy](compatibility_policy.md) and [contract generation](../reference/generation_workflow.md)
- [Execution platform policy](execution_platform_governance.md) and [runtime API stability](runtime_api_stability.md)
- [System glossary](GLOSSARY.md) and [RFCs](rfc/README.md)

## Architectural Decision Records (ADRs)

The core architectural decisions are recorded chronologically in the [ADR directory](adr).

- [ADR-007 memory architecture proposal](ADR_MEMORY.md)

### Foundational Invariants (Expected Stability: Long-term)
* **[ADR-001: Knowledge Runtime Evolution Model](adr/ADR-001-knowledge-runtime-evolution-model.md)** (Accepted) — Invariants governing identity, provenance monotonicity, and reflection purity.
* **[ADR-020: Protocol Independence & Adapter Architecture](adr/ADR-020-protocol-independence.md)** (Proposed) — Establishes strict hexagonal decoupling between Brain Runtime and external interfaces.
* **[ADR-021: Stable Application Interface](adr/ADR-021-stable-application-interface.md)** (Proposed) — Defines the transport-neutral, capability-oriented application interface contract.
* **[ADR-022: Contract Ownership & DTO Generation Strategy](adr/ADR-022-contract-ownership-strategy.md)** (Proposed) — Decides on Rust-first contract ownership and language-neutral type generation workflows.
* **[ADR-023: Shared Adapter Infrastructure](adr/ADR-023-shared-adapter-infrastructure.md)** (Accepted) — Defines the generic, type-erased capability and registry infrastructure for protocol independence.

### Platform & Extensibility (Expected Stability: Medium to High)
* **[ADR-024: IVF Vector Indexing](adr/ADR-024-ivf-vector-indexing.md)** (Accepted) — Establish deterministic inverted file clustering for sub-linear similarity search in SQLite.
* **[ADR-025: Hybrid Retrieval Architecture](adr/ADR-025-hybrid-retrieval-architecture.md)** (Accepted) — Defines independent channels and reciprocal rank fusion (RRF) for hybrid retrieval.
* **[ADR-026: Graph and Projection Capabilities](adr/ADR-026-graph-and-projection-capabilities.md)** (Accepted) — Outlines design invariants for request-scoped graph retrieval, relationship expansion DTOs, and graph/temporal projection view models.

### Operational Lifecycle (Expected Stability: Evolutionary)
* **[ADR-018: Reproducible ML Lifecycle](adr/ADR-018-reproducible-ml-lifecycle.md)** (Proposed) — Defines promotional checkpoints from feedback and evaluation to canary routing.
* **[ADR-019: Observability First](adr/ADR-019-observability-first.md)** (Proposed) — Codifying diagnostic reporting, telemetry tracking, and evaluations as core system capabilities.
