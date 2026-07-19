# ADR-030: Unified Relational Memory Engine Migration Completion

## Status
Accepted (Signed off as of July 16, 2026)

## Context
With the migration from the legacy heuristic-based retrieval engine to the unified native Rust Ratatui TUI and FTS5 BM25 + IVF vector retrieval pipeline completed, we need a permanent document capturing the stable architectural constitution of the system going forward.

This document establishes the structural constraints and extension boundaries that future development must preserve.

---

## The Six Enduring Architectural Invariants

Every future pull request, feature branch, or subsystem refactor must preserve these six fundamental invariants:

### 1. Workflow vs. Capability
Orchestration and flow control are strictly separated from domain implementation details.
*   **Orchestrators** (e.g., `ApplicationRuntime`, `RetrievalService`) own workflow logic—answering *“what happens next”* and coordinating operations.
*   **Capabilities** (e.g., encoders, memory sources, database indices, transporters) handle isolated operations—answering *“how is this executed”* behind clean interfaces.

### 2. Open for Extension
The core execution engine is closed to modification but open to extension. Adding new product features or adapters should occur by authoring new implementations of the existing extension points:
*   `MemorySource` (channels for candidate generation)
*   `RankingStrategy` (reranking and candidate fusion)
*   `QueryEmbeddingService` (semantic query vector generation)
*   `GraphTraversalStrategy` (graph exploration and multihop expansion)
*   `RepositorySet` (underlying durable storage engines)
*   Protocol adapters (network payloads and external boundaries)

If a new feature requires modifying multiple orchestrators, it is a signal that the capability boundaries are incorrect.

### 3. Dependencies Flow Inward
The dependency graph is strictly unidirectional, flowing from external infrastructure interfaces toward the core business domain.
*   **Transport / Adapters** (MCP, HTTP, UDS, A2A) depend on **Application Services** (`ApplicationRuntime`, `RetrievalService`).
*   **Application Services** depend on **Repositories & Interfaces** (`RepositorySet`, `QueryEmbeddingService`).
*   **Repositories** depend on the **Domain Models** (`brain-domain` entities, aggregates, events).
*   No database queries, serialization logic, or dynamic runtime dependencies may leak into the core `brain-domain` crate.

### 4. Evidence Before Optimization
Performance optimization is never based on heuristics or developer assumptions. Every performance modification must follow a strict quality gate:
*   Define a baseline using the test suite or baseline benchmarks (`cargo bench`).
*   Analyze performance changes (latencies, frame draw rates, peak RSS allocations) against the baseline.
*   Prove the improvement objectively using benchmark logs before committing.

### 5. Architectural Invariants Are Documented
ADRs are treated as long-term declarations of structural invariants and systemic rules that survive beyond individual code revisions. They document *why* design constraints exist, rather than detailing ephemeral class names or line-by-line method scopes.

### 6. Single Composition Root
Assembly of the system's object graph occurs exactly once during startup (within `ApplicationRuntime`).
*   No runtime lookups, dynamic dependency resolution, or service locators.
*   No hidden singletons or global state access.
*   Every component must receive its dependencies explicitly via dependency injection (DI).

---

## Future Roadmap: Metadata-First Capability Registry
To scale the extensibility of the engine without violating the **Single Composition Root** invariant, future development will introduce a metadata-first `CapabilityRegistry` instead of a service locator:

*   **Registry as a Catalog**: The registry stores lightweight, metadata-driven `CapabilityFactory` definitions (containing static `CapabilityId` keys and `CapabilityKind` enum classifiers) rather than concrete instances.
*   **Decoupled Instantiation**: Factories construct instances lazily at startup using a shared `CapabilityContext`. This enables early configuration validation, explicit dependency injection, and clean, configuration-driven runtime assembly.

---

## Architectural Review Checklist
When reviewing pull requests that propose substantial additions or modifications to the codebase, the reviewer should verify that the following questions are addressed:

1.  **Invariant Conformance**: Does this work preserve all six core architectural invariants?
2.  **Orchestrator Separation**: Does it introduce a new capability behind an existing/new interface, or does it modify an orchestrator? (Orchestrators should not accumulate capability logic).
3.  **Dependency Alignment**: Are code dependencies flowing strictly inward toward the domain? (No infrastructure details leaking into `brain-domain`).
4.  **Performance Verification**: If the changes affect performance, is there objective benchmark or telemetry evidence proving no regressions?
5.  **Documentation Invariance**: Does this modification warrant an addition or update to the architectural invariants documented in ADRs?

