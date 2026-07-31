---
status: active
owner: plugins
canonical: false
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
subsystem: plugins
owns:
  - crates/brain-plugins
  - crates/brain-python
  - sdks/python
depends_on:
  - domain
used_by:
  - compiler
  - retrieval
canonical_specs:
  - docs/reference/plugin-api.md
adrs:
  - ADR-023
rfcs:
  - RFC-002
---

# Plugin Subsystem Mini-Handbook

> **Governance Role**: This document is a **Navigation Handbook & Subsystem Summary** (`canonical: false`). Canonical plugin trait details live in [`docs/reference/plugin-api.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/plugin-api.md) and extension stability policies live in [`docs/architecture/STABILITY.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/STABILITY.md).

---

## 1. Purpose
The Plugin subsystem allows developers to extend Brain's memory engine with custom Python or Rust semantic extractors, LLM providers, and storage exporters.

## 2. Responsibilities
- Manages the embedded Python runtime via PyO3 / Maturin FFI boundaries.
- Loads dynamic plugin modules from `~/.brain/plugins/`.
- Executes custom `LlmProvider` and `StorageBackend` trait implementations.

## 3. Out of Scope
- Direct SQLite WAL disk writes (owned by **Storage**).
- Terminal viewport rendering (owned by **TUI**).
- Core knowledge reconciliation passes (owned by **Compiler**).

## 4. Architecture Overview
```text
 ┌────────────────────────────────────────────────────────┐
 │                   Brain Runtime (Rust)                  │
 └───────────────────────────┬────────────────────────────┘
                             │ PyO3 / Maturin FFI Boundary
                             ▼
 ┌────────────────────────────────────────────────────────┐
 │                Embedded Python Subsystem               │
 │  - Custom LLM Providers (LlmProvider)                  │
 │  - Semantic Fact Extractors (StorageBackend)           │
 └────────────────────────────────────────────────────────┘
```

## 5. Runtime Flow
1. **Discovery**: On startup, Brain scans `~/.brain/plugins/` for installed Python packages.
2. **FFI Binding**: PyO3 initializes the Python interpreter and binds plugin traits.
3. **Execution**: Brain invokes plugin methods asynchronously within isolated worker threads.

## 6. Key Invariants
- **GIL Safety**: Python GIL locks must be acquired and dropped cleanly without blocking main event loops.
- **Fault Isolation**: Plugin exceptions are caught at the FFI boundary and converted into standard error events.

## 7. Owning Crates
- [`crates/brain-plugins`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-plugins/README.md): Rust plugin traits (`LlmProvider`, `StorageBackend`).
- [`crates/brain-python`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-python/README.md): PyO3 FFI runtime bridge.
- [`sdks/python`](file:///Users/ritikpathania/Developer/PyCharm/brain/sdks/python/README.md): Python client SDK and plugin authoring tools.

## 8. Implementation Notes
- Plugins are built using Maturin and PyO3 version `>= 0.21`.

## 9. Canonical References
- [`docs/reference/plugin-api.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/plugin-api.md): Canonical Python plugin API specification.
- [`docs/architecture/STABILITY.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/STABILITY.md): Stability and extensibility commitments.

## 10. Related ADRs
- [`ADR-023: Shared Adapter Infrastructure`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-023-shared-adapter-infrastructure.md)

## 11. Related RFCs
- [`RFC-002: PyO3 FFI Boundary & Dynamic Python Extraction Plugins`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC-002.md)

## 12. Operations
- Plugin directory: `~/.brain/plugins/`.

## 13. Testing
- Integration tests in `crates/brain-python/tests/` verify Python plugin loading and GIL safety.

## 14. Extension Points
- Implement `LlmProvider` or `StorageBackend` in Python or Rust.

## 15. Subsystem Dependencies
```text
Plugin Subsystem
├── Depends on: Rust Traits (brain-plugins) & FFI Bridge (brain-python)
├── Loaded by: Background Daemon (daemon)
├── Extends: Compiler & Retrieval Pipelines
└── Installed in: ~/.brain/plugins/
```
