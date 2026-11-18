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
  - frontend
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

> **Governance Role**: This document is a **Navigation Handbook & Subsystem Summary** (`canonical: false`). Canonical wire protocol details live in [`docs/reference/protocol.md`](../reference/protocol.md) and contract generation workflows live in [`docs/reference/generation_workflow.md`](../reference/generation_workflow.md).

---

## 1. Purpose
The Protocol subsystem governs IPC communication between clients (first-party
frontend, CLI, SDKs) and the background daemon, as well as HTTP telemetry and
health endpoints.

## 2. Responsibilities
- Defines Data Transfer Object (DTO) contracts and Specta type generation targets.
- Manages Unix Domain Socket (UDS) frame encoding and decoding.
- Serves HTTP Prometheus metrics (`GET /metrics`) and JSON health endpoints (`GET /status`).
- Enforces request versioning (`VersionedRequest`) and workspace context propagation.

The daemon also exposes Fleet registration, requested snapshots and preferences through `v1/fleet/*`. The registry derives requester-relative foreground identity and includes subagent job summaries in snapshots. Pin order and groups persist against stable IDs when the preference store is configured. The maintained frontend's release gate covers its versioned session, generation and interaction contract. [First-party frontend handbook](tui.md)

## 3. Out of Scope
- Domain aggregate logic or entity validations (owned by **Compiler**).
- Terminal rendering, focus, and widgets (owned by the first-party frontend).
- Database migrations or table DDL (owned by **Storage**).

## 4. Architecture Overview
```text
  Clients (first-party frontend / CLI / Python SDK / TS SDK)
                    │
                    ▼
     Unix Domain Socket (daemon.sock)
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
1. The client connects to the effective socket path. `BRAIN_SOCKET_PATH` takes precedence over `daemon.sock` under `BRAIN_CONFIG_DIR` or the default `~/.brain` directory.
2. The client sends a JSON request followed by a newline.
3. The first-party frontend negotiates compatibility with `v1/frontend/handshake` before session and generation operations.
4. The daemon emits newline-terminated JSON responses. Generation frames are correlated by session and generation ID and start their sequence at zero.

## 6. Key Invariants
- **Stream ordering**. Sequence numbers are scoped to one stream. See the [wire reference](../reference/protocol.md) for the search and generation frame formats.
- **Protocol Independence**: Domain logic remains completely decoupled from IPC wire codecs.

## 7. Owning Crates
- [`crates/brain-integrations`](../../crates/brain-integrations/README.md): DTO definitions, Specta contract specs.
- [`daemon`](../../daemon/README.md): UDS server, HTTP Prometheus server.

## 8. Implementation Notes
- Wire DTO types derive `specta::Type` and `serde::Serialize`.

## 9. Canonical References
- [`docs/reference/protocol.md`](../reference/protocol.md): Canonical wire protocol and endpoint specification.
- [`docs/reference/generation_workflow.md`](../reference/generation_workflow.md): Specta contract generation workflow.
- [`docs/architecture/contract-lifecycle.md`](../architecture/contract-lifecycle.md): DTO lifecycle and deprecation policy.

## 10. Related ADRs
- [`ADR-020: Protocol Independence & Adapter Architecture`](../architecture/adr/ADR-020-protocol-independence.md)
- [`ADR-021: Stable Application Interface`](../architecture/adr/ADR-021-stable-application-interface.md)
- [`ADR-022: Contract Ownership & DTO Generation Strategy`](../architecture/adr/ADR-022-contract-ownership-strategy.md)

## 11. Related RFCs
- [`RFC-003: IPC UDS Frame Codecs & Streaming Events Protocol`](../architecture/rfc/RFC-003.md)
- [`RFC-007: Versioned Request Frames`](../architecture/rfc/RFC-007.md)

## 12. Operations
- Prometheus metrics are served at `http://127.0.0.1:8080/metrics` by default. `BRAIN_HEALTH_PORT` overrides the port.

## 13. Testing
- Integration tests in `daemon/tests/` verify UDS request-response frames and HTTP endpoints.

## 14. Extension Points
- Add new request payloads to `crates/brain-integrations/src/lib.rs` and run `cargo xtask generate-contracts`.

## 15. Subsystem Dependencies
```text
Protocol Subsystem
├── Depends on: Integration DTOs (brain-integrations)
├── Hosted by: Daemon (daemon)
├── Receives requests from: first-party frontend, CLI, Python/TS SDKs
└── Forwards commands to: Application Services (brain-application)
```
