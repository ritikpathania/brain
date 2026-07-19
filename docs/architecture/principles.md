# Architectural Principles

This document defines the enduring engineering rules that guide the implementation and evolution of the Brain system. Every future pull request and design proposal must conform to these principles.

---

## 1. Single Authoritative Runtime
All knowledge lifecycle actions (ingestion, canonicalization, semantic reflection, and read-model projections) belong strictly to `BrainRuntime`. No other subsystem is permitted to make structural decisions about knowledge or state.

## 2. Stateless Transport Adapters
Transport adapters (such as MCP, UDS IPC, HTTP metrics, or A2A components) are pure, stateless boundaries. They convert wire protocol formats and propagate operations to the runtime, holding no domain state and executing no business logic directly.

## 3. Storage Behind Interfaces
All persistent domain operations are executed via clean trait boundaries (e.g. `Storage`, `SearchRepository`, `NodeRepository`). The implementation (currently SQLite) is completely isolated from the execution engine, ensuring infrastructure changes do not bleed into the core logic.

## 4. Composition Over Duplication
Capabilities are built entirely by extending or composing unified runtime projections (such as `SearchProjector` and `MemoryListProjection`) rather than introducing separate, bespoke query pipelines.

## 5. Runtime Owns Knowledge
The representation schema of entities, relation types, and canonicalization constraints is strictly owned by the runtime. The runtime makes the final decision on how observations are structured and cataloged.

## 6. Adapters Own Protocols
Adapters own the schemas and serialization formats required by external callers (such as JSON-IPC envelopes). The core business domain and models are independent of any serialization annotations or transport specifics.
