# Project Roadmap

This document outlines the high-level roadmap and architectural direction for the Brain Relational Memory Engine.

---

## 🎯 Version 1.0 (Current Release - Stable Core)

- [x] **Composition Root**: `BrainRuntime` single authoritative composition root.
- [x] **Storage Engine**: SQLite FTS5 lexical + vector BLOB hybrid search engine.
- [x] **Reconciliation Engine**: 6-pass deterministic knowledge compiler (`crates/brain-services`).
- [x] **IPC Daemon**: Unix Domain Socket (UDS) background daemon with streaming frames.
- [x] **Terminal UI (TUI)**: Native Ratatui client with command palette, typewriter queue, and theme system.
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

## 🔮 Version 2.0 (Future Vision)

- [ ] **Distributed Consensus (Raft)**: Active-active multi-node replication across remote daemon instances.
- [ ] **Dynamic Plugin Sandbox**: WASM-based plugin runtime for safe, sandboxed custom graph reflection passes.
- [ ] **Self-Optimizing Indexing**: Machine-learning driven automated pruning and re-ranking based on retrieval telemetry.
