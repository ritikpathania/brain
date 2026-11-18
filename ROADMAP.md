# Project Roadmap

This document outlines the high-level roadmap and architectural direction for the Brain Relational Memory Engine.

---

## 🎯 Version 1.0 (Stable Core)

- [x] **Composition Root**: `BrainRuntime` single authoritative composition root.
- [x] **Storage Engine**: SQLite FTS5 lexical + vector BLOB hybrid search engine.
- [x] **Reconciliation Engine**: Deterministic knowledge compiler with 18 passes across six phases (`crates/brain-services`).
- [x] **IPC Daemon**: Unix Domain Socket (UDS) background daemon with streaming frames.
- [x] **Observability**: Prometheus metrics exposition and HTTP health endpoints (`:8080`).
- [x] **Quality Gate**: Comprehensive `cargo xtask verify` contract and architecture verification.

---

## 🚀 Version 1.1 (Retrieval, Runtime and First-Party Frontend)

- [x] **Scoped generation context**. Stored constraints take priority over query-dependent lexical, vector and graph candidates within per-turn budgets. The first-party adapter consumes allowlisted context metadata.
- [x] **Native extraction and providers**. Runtime startup configures native heuristic and LLM extraction, chat providers and embeddings. Embedded Python remains a separate plugin integration.
- [x] **Runtime composition**. Share engine and compiler state through the application facade, hydrate the compiler from storage, and coordinate application and engine shutdown.
- [x] **Personalization evaluation**. Measure joint All-Facts Recall@K and hybrid complementarity on scoped fixture collections. The live generation context recognizes the same persisted constraint flags.
- [x] **Fleet backend**. The daemon supports a foreground registration and requested snapshot API with durable pin ordering and groups.
- [x] **Reasoning Reflection Pipeline**: Phase 6 reflection, candidate extraction, graph matching, and memory stewardship facade.
- [ ] **Expanded Vector Indexing**: Support HNSW vector indexing and evaluate write-time projection caching for sub-millisecond similarity queries over more than one million embeddings.
- [ ] **Multi-Session Isolation**: Fine-grained session namespace partitioning for concurrent workspace agents.
- [ ] **SDK Package Distribution**:
  - Publish the TypeScript SDK to npm and the Python SDK to PyPI after confirming the public package names.
  - Publish `brain-sdk-rs` to crates.io.
- [ ] **MCP adapter enhancement**. Add tool-call streaming and resource subscriptions, and verify deployed-peer interoperability.

---

## First-party frontend and product delivery

- [x] **Canonical launch**. `brain ui` resolves the first-party `packages/brain-frontend` from a development checkout or validates the standalone executable in an installed product bundle.
- [x] **Daemon integration**. The frontend uses the versioned handshake, session, generation, tool, and permission contracts over UDS.
- [ ] **Knowledge exploration**. Evaluate graph search, match explanations, and node inspection in the first-party frontend.
- [ ] **Compiler and reflection diagnostics**. Evaluate surfacing compiler and memory consolidation results through the first-party adapter.
- [ ] **Release acceptance**. Verify the exact source and extracted archives on macOS arm64 and Linux x86_64 for each release candidate. [Distribution guide](docs/guides/product-distribution.md#4-platform-verification-status)

---

## 🔮 Version 2.0 (Future Vision)

- [ ] **Distributed Consensus (Raft)**: Active-active multi-node replication across remote daemon instances.
- [ ] **Dynamic Plugin Sandbox**: WASM-based plugin runtime for safe, sandboxed custom graph reflection passes.
- [ ] **Self-optimizing indexing**. Evaluate learned pruning and re-ranking from retrieval telemetry.
- [ ] **Retrieval of failed actions**. Evaluate using failed tool outcomes and reverted edits to reduce repeated actions across turns.


## Acceptance for planned work

| Milestone | Acceptance evidence |
| --- | --- |
| Retrieval indexing and ranking | Compare against the maintained ranking baselines. Record corpus size, query and write latency, and equivalent retrieval quality at the stated scale before promoting an index or model. |
| Session isolation and frontend capabilities | Show replay-safe, requester-scoped behavior in daemon integration and frontend state/PTY tests. Exercise keyboard navigation, session changes and restart recovery where applicable. |
| SDK distribution and adapter interoperability | Install each artifact into a clean consumer, run version/contract checks, and exercise the supported adapter handshake and operations against the same daemon version. |
| Distributed execution and plugin isolation | Review the phase design and exercise failure, restart and concurrency scenarios before advertising the capability. |
