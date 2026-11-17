# Project Roadmap

This document outlines the high-level roadmap and architectural direction for the Brain Relational Memory Engine.

---

## 🎯 Version 1.0 (Current Release - Stable Core)

- [x] **Composition Root**: `BrainRuntime` single authoritative composition root.
- [x] **Storage Engine**: SQLite FTS5 lexical + vector BLOB hybrid search engine.
- [x] **Reconciliation Engine**: Deterministic knowledge compiler with 18 passes across six phases (`crates/brain-services`).
- [x] **IPC Daemon**: Unix Domain Socket (UDS) background daemon with streaming frames.
- [x] **Terminal UI (historical)**: The original Ratatui milestone covered the command palette, typewriter queue, and theme system.
- [x] **Observability**: Prometheus metrics exposition and HTTP health endpoints (`:8080`).
- [x] **Quality Gate**: Comprehensive `cargo xtask verify` contract and architecture verification.

---

## 🚀 Version 1.1 (Current Release - Retrieval & Knowledge Exploration)

- [x] **Retrieval UI Projection Pipeline**: Sub-150ms debounce, localized cancellation ownership, stable selection invariant.
- [x] **Knowledge Exploration Architecture**: synchronized graph/list projections, navigation session stack, explainability & provenance.
- [x] **Reasoning Reflection Pipeline**: Phase 6 reflection, candidate extraction, graph matching, and memory stewardship facade.
- [ ] **Expanded Vector Indexing**: Support HNSW vector indexing for sub-millisecond similarity queries over >1M embeddings.
- [ ] **Multi-Session Isolation**: Fine-grained session namespace partitioning for concurrent workspace agents.
- [ ] **SDK Package Distribution**:
  - Publish `@brain-engine/sdk` to npm.
  - Publish `brain-engine` to PyPI.
  - Publish `brain-sdk-rs` to crates.io.
- [ ] **MCP Adapter Enhancement**: Full support for Model Context Protocol (MCP) tool call streaming & resource subscriptions.

---

## First-party frontend and product delivery

- [x] **Canonical launch**. `brain ui` resolves the first-party `packages/brain-frontend` from a development checkout or validates the standalone executable in an installed product bundle.
- [x] **Daemon integration**. The frontend uses the versioned handshake, session, generation, tool, and permission contracts over UDS.
- [x] **Phase 5 platform release verification**. The candidate passed native archive verification on macOS arm64 and Linux x86_64. Each subsequent release candidate requires verification of its own archives. [Distribution guide](docs/guides/product-distribution.md#4-platform-verification-status)
- [ ] **Legacy retirement (Phase 6, on hold)**. Retain `packages/brain-shell` and reference assets until the required product verification and final caller inventory support a separate retirement decision.

---

## 🔮 Version 2.0 (Future Vision)

- [ ] **Distributed Consensus (Raft)**: Active-active multi-node replication across remote daemon instances.
- [ ] **Dynamic Plugin Sandbox**: WASM-based plugin runtime for safe, sandboxed custom graph reflection passes.
- [ ] **Self-Optimizing Indexing**: Machine-learning driven automated pruning and re-ranking based on retrieval telemetry.
