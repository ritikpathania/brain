# Brain Roadmap & Task Checklist

## Compliance Checklist
Before completing any task or opening a PR, verify the following:
- `[x]` **Transport-Neutrality**: Does this change preserve the transport-neutral event model?
- `[x]` **Opaque Metadata**: Does it introduce provider-specific logic into Brain? (If yes, reject.)
- `[x]` **Write-Ahead Log**: Does it bypass the event log?
- `[x]` **Immutability**: Does it mutate accepted events?
- `[x]` **Dependency Graph Hygiene**: Does it violate crate dependency rules (e.g. upward dependencies)?
- `[x]` **Schema Alignment**: Does it change the canonical schema?
- `[x]` **Rule of Two**: Is every new abstraction justified by at least two concrete implementations?

---

## Roadmap Checkpoints

### 1. Product (User Experience & Capabilities)
*User-facing features, query capabilities, and UX interfaces.*

- [x] Define canonical `protocol/brain_events.schema.json` schema reference.
- [x] Ingest payload, validate schema, verify deduplication using `event_id`.
- [x] Persist to `event_log` and assign sequence number.
- [x] Return sequence number & ACK immediately after WAL persistence.
- [x] Implement replay endpoint utilizing `ReplayPosition` sequence comparison.
- [ ] Better Semantic Retrieval: Hybrid search, knowledge graph traversal, and query optimization.
- [ ] Memory Consolidation: Ranking models and reflection improvement loops.
- [ ] CLI UX: Usability improvements, command aliases, better formatting, and error handling.
- [ ] TUI Improvements: Rich terminal workflows, scheduling visualizer, and interactive exploration.

---

### 2. Platform (Runtime Infrastructure & Stability)
*Runtime lifecycle, performance, packaging, distribution, and operational metrics.*

- [x] Initialize new workspace crate `crates/brain-integrations/`.
- [x] Write exhaustive serialization and validation tests (Rust DTO round-trips, schema conformance).
- [x] **Golden-File Tests**: Add a permanent compatibility suite verifying JSON -> DTO -> JSON canonical serialization.
- [x] Create SQLite table `event_log` and migrations in `brain-storage`.
- [x] Implement `ingest_event` request handler action in daemon.
- [x] **Refactor Downcast**: Abstract the daemon's storage access to an explicit capability interface (e.g. `EventLogRepository`).
- [x] **Protocol Fuzzing**: Implement a fuzz test generating random valid/invalid envelopes.
- [x] Create `crates/brain-sdk-rs/` workspace crate.
- [x] Implement connection lifecycle with automatic reconnection and backoff.
- [x] Implement local tracking and replay on reconnection to guarantee zero event loss.
- [x] Implement synchronous and asynchronous `send` SDK interfaces.
- [x] Build TypeScript Client SDK with byte-level Transport abstraction.
- [x] Build `brain-application` crate exposing a capability-oriented boundary.
- [x] Formalize Application Interface version policy (`1.0.0`) and compatibility range.
- [ ] **Background Daemon Lifecycle & Socket Recovery**: Graceful shutdown signals (SIGINT/SIGTERM), worker draining, and stale UDS socket recovery.
- [ ] Observability: Performance profiling, telemetry tracking, and latency metrics.
- [ ] Packaging & Release Automation: Build pipelines, packaging, and security hardening.
- [ ] REST Adapter: Build REST API adapter (only when a concrete HTTP consumer exists).

---

### 3. Integrations (Ecosystem Extensibility)
*Lower priority extensibility and automation triggers.*

- [x] Build CLI Adapter (`brain-cli-adapter`) utilizing `brain-sdk-rs` to stream stdin/args.
- [x] Build MCP Adapter (`brain-mcp-adapter`) for dynamic JSON-RPC capability exposure.
- [x] Build ACP Adapter (`brain-acp-adapter`) for agent clients.
- [x] Build A2A Adapter (`brain-a2a-adapter`) for agent-to-agent message passing.
- [ ] Automation Hooks: Integrate workflow dispatch triggers (when concrete use cases exist).

---

## Operational Success Metrics

### Product
*   **Retrieval Quality**: Mean Reciprocal Rank (MRR) and Recall@K.
*   **Latency**: P95 and P99 latency bounds for search/retrieval.
*   **Usability**: Zero-friction setup and clear CLI command discovery.

### Platform
*   **Startup Time**: Daemon launch and UDS bind latency.
*   **Recovery Rate**: 100% recovery of UDS socket on stale socket files or crashes.
*   **Memory Usage**: Peak RSS memory tracking under load.
*   **Reliability**: Zero event loss on network reconnection.
