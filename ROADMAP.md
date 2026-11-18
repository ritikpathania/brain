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
- [x] **Phase 6 caller inventory**. The product and release workflows use the first-party frontend. Manual PTY scripts, local launch and oracle tooling, and frozen assets still depend on the legacy tree. [Caller inventory](docs/designs/2026-11-17_DRIFT_REPORT.md)
- [x] **Phase 6 legacy frontend retirement**. The tracked shell and its manual PTY entry points are retired. Historical source, snapshots, and local oracle receipts remain available for reproduction. [Phase 6 design and acceptance plan](docs/designs/phase-6-legacy-frontend-retirement.md)

---

## 🔮 Version 2.0 (Future Vision)

- [ ] **Distributed Consensus (Raft)**: Active-active multi-node replication across remote daemon instances.
- [ ] **Dynamic Plugin Sandbox**: WASM-based plugin runtime for safe, sandboxed custom graph reflection passes.
- [ ] **Self-Optimizing Indexing**: Machine-learning driven automated pruning and re-ranking based on retrieval telemetry.

## Phase 6 release follow-through

- [x] Verify the synchronized ordinary CI consumer with executable archive fixtures.
- [x] Complete the native Linux and macOS archive matrix for the final Phase 6 release candidate, with exact source and artifact evidence.
- [x] Synchronize the accepted verification branch while preserving unrelated original working files.

The maintainer's local Rust 1.99.1 toolchain path currently resolves through a symlink to stable Rust 1.100.0. The [distribution guide](docs/guides/product-distribution.md) distinguishes this local provenance from the CI pin.

Native follow-through product source `8a6d020d` passed both platforms on 2026-11-18. [Exact acceptance evidence](docs/guides/product-distribution.md#phase-6-native-archive-acceptance--2026-11-18). Future roadmap features remain separate from this completed release-verification increment.
