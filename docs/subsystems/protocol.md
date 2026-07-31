---
status: active
owner: protocol
canonical: false
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
subsystem: protocol
owns:
  - crates/brain-integrations
  - daemon
depends_on:
  - application
used_by:
  - tui
  - sdk
canonical_specs:
  - docs/reference/protocol.md
  - docs/reference/generation_workflow.md
adrs:
  - ADR-020
  - ADR-021
  - ADR-022
rfcs:
  - RFC-003
  - RFC-007
---

# IPC & Wire Protocol Subsystem Mini-Handbook

> **Governance Role**: This document is a **Navigation Handbook & Subsystem Summary** (`canonical: false`). Canonical wire protocol details live in [`docs/reference/protocol.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/protocol.md) and contract generation workflows live in [`docs/reference/generation_workflow.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/generation_workflow.md).

---

## 1. Purpose
The Protocol subsystem governs IPC communication between clients (TUI, CLI, SDKs) and the background daemon, as well as HTTP telemetry and health endpoints.

## 2. Responsibilities
- Defines Data Transfer Object (DTO) contracts and Specta type generation targets.
- Manages Unix Domain Socket (UDS) frame encoding and decoding (`~/.brain/brain.sock`).
- Serves HTTP Prometheus metrics (`GET /metrics`) and JSON health endpoints (`GET /status`).
- Enforces request versioning (`VersionedRequest`) and workspace context propagation.

## 3. Out of Scope
- Domain aggregate logic or entity validations (owned by **Compiler**).
- Terminal viewport display or widget drawing (owned by **TUI**).
- Database migrations or table DDL (owned by **Storage**).

## 4. Architecture Overview
```text
  Clients (TUI / CLI / Python SDK / TS SDK)
                    │
                    ▼
     Unix Domain Socket (~/.brain/brain.sock)
                    │
                    ▼
 ┌─────────────────────────────────────────────────────┐
 │                    Daemon Server                    │
 ├──────────────────────────┬──────────────────────────┤
 │ UDS Frame Codec          │ HTTP Telemetry Listener  │
 │ - VersionedRequest       │ - GET /metrics (Prom)    │
 │ - StreamEvent (Monotonic)│ - GET /metrics/json      │
 └──────────────────────────┴──────────────────────────┘
```

## 5. Runtime Flow
1. **Connect**: Client connects to `~/.brain/brain.sock`.
2. **Frame Dispatch**: Client sends a length-prefixed JSON `VersionedRequest`.
3. **Stream Events**: Daemon streams monotonic `StreamEvent` variants (`stream_start`, `stream_chunk`, `stream_end`).

## 6. Key Invariants
- **Monotonic Sequence Numbers**: `StreamEvent` sequence numbers increment monotonically within a session.
- **Protocol Independence**: Domain logic remains completely decoupled from IPC wire codecs.

## 7. Owning Crates
- [`crates/brain-integrations`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-integrations/README.md): DTO definitions, Specta contract specs.
- [`daemon`](file:///Users/ritikpathania/Developer/PyCharm/brain/daemon/README.md): UDS server, HTTP Prometheus server.

## 8. Implementation Notes
- Wire DTO types derive `specta::Type` and `serde::Serialize`.

## 9. Canonical References
- [`docs/reference/protocol.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/protocol.md): Canonical wire protocol and endpoint specification.
- [`docs/reference/generation_workflow.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/reference/generation_workflow.md): Specta contract generation workflow.
- [`docs/architecture/contract-lifecycle.md`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/contract-lifecycle.md): DTO lifecycle and deprecation policy.

## 10. Related ADRs
- [`ADR-020: Protocol Independence & Adapter Architecture`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-020-protocol-independence.md)
- [`ADR-021: Stable Application Interface`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-021-stable-application-interface.md)
- [`ADR-022: Contract Ownership & DTO Generation Strategy`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-022-contract-ownership-strategy.md)

## 11. Related RFCs
- [`RFC-003: IPC UDS Frame Codecs & Streaming Events Protocol`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC-003.md)
- [`RFC-007: Versioned Request Frames`](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC-007.md)

## 12. Operations
- Prometheus metric scraping path: `http://localhost:9090/metrics`.

## 13. Testing
- Integration tests in `daemon/tests/` verify UDS request-response frames and HTTP endpoints.

## 14. Extension Points
- Add new request payloads to `crates/brain-integrations/src/lib.rs` and run `cargo xtask generate-contracts`.

## 15. Subsystem Dependencies
```text
Protocol Subsystem
├── Depends on: Integration DTOs (brain-integrations)
├── Hosted by: Daemon (daemon)
├── Receives requests from: TUI, CLI, Python/TS SDKs
└── Forwards commands to: Application Services (brain-application)
```
