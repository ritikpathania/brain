---
status: active
owner: tui
canonical: false
review_cycle: quarterly
last_reviewed: 2026-09-30
applies_to: v1.1+
subsystem: tui
owns:
  - frontend integration boundary
depends_on:
  - protocol
  - sessions
used_by:
  - cli
canonical_specs:
  - docs/reference/protocol.md
  - docs/architecture/principles.md
adrs: []
rfcs: []
---

# Canonical Frontend Integration Handbook

> **Governance role:** This is an integration handbook, not a visual-design authority. The canonical development frontend is the independently authored first-party `packages/brain-frontend`.

## Purpose

Brain's terminal product boundary is:

```text
Brain CLI and services
  → versioned UDS frontend protocol
  → owned TypeScript Brain adapter/projection
  → frontend-neutral state
  → first-party Brain terminal presentation
```

The CLI resolves `packages/brain-frontend` from the Brain repository containing its executable. `BRAIN_FRONTEND_ROOT` is an optional explicit development override, never an automatic home-directory/private-checkout search. The validated package root becomes the child working directory. `packages/brain-shell` is legacy migration/reference code and is not the `brain ui` destination.

## Responsibilities

- Resolve the first-party development package (or explicit override) and launch it with the effective daemon socket configuration.
- Require `v1/frontend/handshake` before Brain-backed work is usable.
- Keep Brain authoritative for sessions, transcript history, generation lifecycle, tool execution, permission settlement, reconnect reconciliation, and retrieval metadata.
- Map only versioned protocol DTOs into the frontend adapter; Rust domain/storage structures do not enter presentation components.
- Keep rendering, keyboard, focus, scrolling, and terminal behavior in the frontend presentation/input layer, above protocol and neutral state.

## Runtime flow

```text
brain ui
  → explicit BRAIN_FRONTEND_ROOT override, otherwise executable-relative repository package
  → ensure/connect daemon
  → launch frontend at that root
  → frontend adapter connects over UDS
  → v1/frontend/handshake negotiates protocol and capabilities
  → Brain-backed session and generation operations become available
```

An invalid override or unavailable repository package is an actionable nonzero launch failure. The manifest must identify `@brain/frontend`, provide a version, and start `bun run src/cli/main.ts`. Bun is resolved from `PATH` or explicit `BRAIN_FRONTEND_RUNTIME`; absence is an error, not an installation or fallback request. After daemon preflight, Unix terminal ownership is handed to Bun, preserving exit/signal status.

## Operational boundaries

- Canonical CI verifies the owned in-repo package; it does not fetch external/private source.
- Packaged artifact resolution and full product delivery are implemented and verified via closed manifests and clean-extraction PTY smoke proof (see `docs/guides/product-distribution.md`).
- The frontend does not execute Brain tools locally. Brain owns tool identity, permission resolution, process lifecycle, and results.
- Live generation uses lexical search, bounded graph expansion, and vector recall when a configured provider and stored embeddings are available. Visible pinned constraints take priority within the item and token budgets.
- Retrieval metadata uses the allowlisted `brain_context` contract. If context assembly fails, the daemon logs the failure and continues generation with the field absent. The adapter clears a prior summary only after an explicit successful snapshot with zero results.

## Verification

The launcher and daemon protocol suites verify discovery, handshake, sessions/history, streams, tools/permissions, reconnect, and safe context metadata. CI requires frozen Bun installation, full package typecheck, owned unit/state/render tests, and isolated real-daemon E2E including actual `brain ui` PTY launch/shutdown. Missing daemon/CLI prerequisites fail; required tests do not skip. Daemon-side Bash tests retain process-group timeout/TERM/KILL/reap proof rather than duplicating it in the UI. Production standalone frontend build, artifact verification, and clean-extraction interactive PTY smoke are verified in the product packaging orchestrator.

## References

- [UDS protocol reference](../reference/protocol.md)
- [Architecture principles](../architecture/principles.md)
- [Audit 2](../audits/PROJECT_AUDIT_2.md) — historical evidence and remaining release blockers
